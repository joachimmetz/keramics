/* Copyright 2024-2026 Joachim Metz <joachim.metz@gmail.com>
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License. You may
 * obtain a copy of the License at https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS, WITHOUT
 * WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied. See the
 * License for the specific language governing permissions and limitations
 * under the License.
 */

//! LZMA decompression.
//!
//! Provides decompression support for LZMA (7-Zip/LZMA1) compressed data.
//!
//! The supported format is the standalone LZMA stream (also called
//! LZMA-alone): a 13-byte stream header consisting of the properties byte,
//! the 32-bit little-endian dictionary size and the 64-bit little-endian
//! uncompressed data size, followed by the range-coded LZMA bitstream.

use keramics_core::{DebugTrace, DebugTraceScope, ErrorTrace};
use keramics_layout_map::LayoutMap;
use keramics_types::{bytes_to_u32_le, bytes_to_u64_le};

/// Maximum value of the LZMA properties byte ((4 * 5 + 4) * 9 + 8).
const LZMA_MAX_PROPERTIES: u8 = 224;

/// Minimum LZMA dictionary size.
const LZMA_MINIMUM_DICTIONARY_SIZE: usize = 1 << 12;

/// Value of the uncompressed data size used to indicate that the uncompressed data size is unknown.
const LZMA_UNCOMPRESSED_SIZE_UNKNOWN: u64 = u64::MAX;

/// LZMA end of payload marker (EOPM) match distance value.
const LZMA_END_OF_PAYLOAD_MARKER: u32 = 0xFFFFFFFF;

/// Number of LZMA decoder states.
const LZMA_NUMBER_OF_STATES: usize = 12;

/// Number of LZMA literal states (states 0 to 6 indicate that the previous state was a literal).
const LZMA_NUMBER_OF_LITERAL_STATES: usize = 7;

/// Number of LZMA position states (1 << 4).
const LZMA_POSITION_STATES: usize = 16;

/// Number of LZMA distance slot states.
const LZMA_NUMBER_OF_DISTANCE_SLOT_STATES: usize = 4;

/// Number of LZMA distance slots (1 << 6).
const LZMA_NUMBER_OF_DISTANCE_SLOTS: usize = 64;

/// First LZMA distance slot that uses the distance low bits table.
const LZMA_DISTANCE_SLOT_LOW_BITS: usize = 4;

/// First LZMA distance slot that uses plain and align bits.
const LZMA_DISTANCE_SLOT_PLAIN_BITS: usize = 14;

/// Number of LZMA distance align bits.
const LZMA_NUMBER_OF_DISTANCE_ALIGN_BITS: usize = 4;

/// Number of LZMA match length low symbols (1 << 3).
const LZMA_MATCH_LENGTH_LOW_SYMBOLS: usize = 8;

/// Minimum LZMA match length.
const LZMA_MATCH_LENGTH_MINIMUM: usize = 2;

/// Number of LZMA match length mid symbols (1 << 3).
const LZMA_MATCH_LENGTH_MID_SYMBOLS: usize = 8;

/// Number of LZMA match length high symbols (1 << 8).
const LZMA_MATCH_LENGTH_HIGH_SYMBOLS: usize = 256;

/// Number of probability cells used by a match length decoder.
///
/// Consists of: choice (1), choice2 (1), low (16 * 8), mid (16 * 8) and high (256) cells.
const LZMA_MATCH_LENGTH_NUMBER_OF_CELLS: usize = 2
    + (LZMA_POSITION_STATES * LZMA_MATCH_LENGTH_LOW_SYMBOLS)
    + (LZMA_POSITION_STATES * LZMA_MATCH_LENGTH_MID_SYMBOLS)
    + LZMA_MATCH_LENGTH_HIGH_SYMBOLS;

/// Number of LZMA probability bit model total bits.
const LZMA_PROBABILITY_TOTAL_BITS: u32 = 11;

/// Number of LZMA probability bit model bits.
const LZMA_PROBABILITY_TOTAL: u16 = 1 << LZMA_PROBABILITY_TOTAL_BITS;

/// Initial value of an LZMA probability bit model.
const LZMA_PROBABILITY_INIT: u16 = LZMA_PROBABILITY_TOTAL >> 1;

/// Number of bits used when normalizing an LZMA probability bit model.
const LZMA_PROBABILITY_MOVE_BITS: u32 = 5;

/// Number of LZMA range coder top value bits.
const LZMA_RANGE_CODER_TOP_BITS: u32 = 24;

/// LZMA range coder top value.
const LZMA_RANGE_CODER_TOP_VALUE: u32 = 1 << LZMA_RANGE_CODER_TOP_BITS;

/// Decodes a bit using a probability bit model.
fn lzma_decode_bit(
    range_decoder: &mut LzmaRangeDecoder,
    prob: &mut u16,
) -> Result<u32, ErrorTrace> {
    range_decoder.normalize()?;
    let bound: u32 = (range_decoder.range >> LZMA_PROBABILITY_TOTAL_BITS) * *prob as u32;

    if range_decoder.code < bound {
        range_decoder.range = bound;
        *prob += (LZMA_PROBABILITY_TOTAL - *prob) >> LZMA_PROBABILITY_MOVE_BITS;

        Ok(0)
    } else {
        range_decoder.range -= bound;
        range_decoder.code -= bound;
        *prob -= *prob >> LZMA_PROBABILITY_MOVE_BITS;

        Ok(1)
    }
}

/// Decodes a match length (2 to 273) using a match length decoder.
fn lzma_decode_length(
    range_decoder: &mut LzmaRangeDecoder,
    position_state: usize,
    table: &mut [u16],
) -> Result<usize, ErrorTrace> {
    let base_index: usize;
    let limit: usize;
    let offset: usize;

    if lzma_decode_bit(range_decoder, &mut table[0])? == 0 {
        // 2 + low value (0 to 7).
        base_index = 2 + (position_state * LZMA_MATCH_LENGTH_LOW_SYMBOLS);
        limit = LZMA_MATCH_LENGTH_LOW_SYMBOLS;
        offset = 0;
    } else {
        if lzma_decode_bit(range_decoder, &mut table[1])? == 0 {
            // 2 + 8 + mid value (0 to 7).
            base_index = 2
                + (LZMA_POSITION_STATES * LZMA_MATCH_LENGTH_LOW_SYMBOLS)
                + (position_state * LZMA_MATCH_LENGTH_MID_SYMBOLS);
            limit = LZMA_MATCH_LENGTH_MID_SYMBOLS;
            offset = LZMA_MATCH_LENGTH_LOW_SYMBOLS;
        } else {
            // 2 + 8 + 8 + high value (0 to 255).
            base_index = 2
                + (LZMA_POSITION_STATES * LZMA_MATCH_LENGTH_LOW_SYMBOLS)
                + (LZMA_POSITION_STATES * LZMA_MATCH_LENGTH_MID_SYMBOLS);
            limit = LZMA_MATCH_LENGTH_HIGH_SYMBOLS;
            offset = LZMA_MATCH_LENGTH_LOW_SYMBOLS + LZMA_MATCH_LENGTH_MID_SYMBOLS;
        }
    }
    let mut symbol: usize = 1;

    while symbol < limit {
        let bit: u32 = lzma_decode_bit(range_decoder, &mut table[base_index + symbol])?;

        symbol = (symbol << 1) + bit as usize;
    }
    Ok(LZMA_MATCH_LENGTH_MINIMUM + (symbol - limit) + offset)
}

/// Bitstream of an LZMA range coder.
struct LzmaRangeDecoder<'a> {
    /// Byte stream.
    data: &'a [u8],

    /// Current offset in the byte stream.
    data_offset: usize,

    /// Size of the byte stream in bytes.
    data_size: usize,

    /// Code value.
    code: u32,

    /// Range value.
    range: u32,
}

impl<'a> LzmaRangeDecoder<'a> {
    /// Creates a new range decoder and initializes it with the first 5 bytes of the byte stream.
    pub fn new(data: &'a [u8]) -> Result<Self, ErrorTrace> {
        if data.len() < 5 {
            return Err(keramics_core::error_trace_new!(
                "Invalid compressed data value too small"
            ));
        }
        if data[0] != 0 {
            return Err(keramics_core::error_trace_new!(format!(
                "Invalid range coder initialization byte: 0x{:02x} (expected: 0x00)",
                data[0]
            )));
        }
        let code: u32 = ((data[1] as u32) << 24)
            | ((data[2] as u32) << 16)
            | ((data[3] as u32) << 8)
            | (data[4] as u32);

        Ok(Self {
            data,
            data_offset: 5,
            data_size: data.len(),
            code,
            range: 0xFFFFFFFF,
        })
    }

    /// Normalizes the range and code values.
    pub fn normalize(&mut self) -> Result<(), ErrorTrace> {
        if self.range < LZMA_RANGE_CODER_TOP_VALUE {
            if self.data_offset >= self.data_size {
                return Err(keramics_core::error_trace_new!(
                    "Invalid compressed data value too small"
                ));
            }
            self.range <<= 8;
            self.code = (self.code << 8) | self.data[self.data_offset] as u32;
            self.data_offset += 1;
        }
        Ok(())
    }

    /// Decodes a direct bit without using a probability bit model.
    pub fn decode_direct_bit(&mut self) -> Result<u32, ErrorTrace> {
        self.normalize()?;
        self.range >>= 1;

        if self.code >= self.range {
            self.code -= self.range;

            Ok(1)
        } else {
            Ok(0)
        }
    }

    /// Determines whether the range coder is at the end of the bitstream.
    pub fn is_finished(&self) -> bool {
        self.code == 0
    }
}

/// Stream header used by LZMA compressed data.
#[derive(LayoutMap)]
#[layout_map(
    structure(
        byte_order = "little",
        field(name = "properties", data_type = "u8"),
        field(name = "dictionary_size", data_type = "u32"),
        field(name = "uncompressed_data_size", data_type = "u64"),
    ),
    methods("debug_read_data")
)]
struct LzmaStreamHeader {
    /// Number of literal context bits (lc).
    literal_context_bits: u8,

    /// Number of literal position bits (lp).
    literal_pos_bits: u8,

    /// Number of position bits (pb).
    pos_bits: u8,

    /// Dictionary size.
    dictionary_size: usize,

    /// Uncompressed data size.
    uncompressed_data_size: u64,
}

impl LzmaStreamHeader {
    /// Creates a new stream header.
    pub fn new() -> Self {
        Self {
            literal_context_bits: 0,
            literal_pos_bits: 0,
            pos_bits: 0,
            dictionary_size: 0,
            uncompressed_data_size: 0,
        }
    }

    /// Reads the stream header.
    pub fn read_data(&mut self, data: &[u8]) -> Result<(), ErrorTrace> {
        if data.len() < 13 {
            return Err(keramics_core::error_trace_new!("Unsupported data size"));
        }
        let properties: u8 = data[0];

        if properties > LZMA_MAX_PROPERTIES {
            return Err(keramics_core::error_trace_new!(format!(
                "Unsupported properties value: 0x{:02x}",
                properties
            )));
        }
        let mut properties_value: usize = properties as usize;

        self.literal_context_bits = (properties_value % 9) as u8;
        properties_value /= 9;
        self.literal_pos_bits = (properties_value % 5) as u8;
        self.pos_bits = (properties_value / 5) as u8;

        if (self.literal_context_bits + self.literal_pos_bits) > 4 {
            return Err(keramics_core::error_trace_new!(format!(
                "Unsupported properties value: 0x{:02x} (lc + lp > 4)",
                properties
            )));
        }
        let mut dictionary_size: usize = bytes_to_u32_le!(data, 1) as usize;

        if dictionary_size < LZMA_MINIMUM_DICTIONARY_SIZE {
            dictionary_size = LZMA_MINIMUM_DICTIONARY_SIZE;
        }
        self.dictionary_size = dictionary_size;
        self.uncompressed_data_size = bytes_to_u64_le!(data, 5);

        Ok(())
    }
}

/// Context for decompressing LZMA compressed data.
pub struct LzmaContext {
    /// Uncompressed data size.
    pub uncompressed_data_size: usize,
}

impl LzmaContext {
    /// Creates a new context.
    pub fn new() -> Self {
        Self {
            uncompressed_data_size: 0,
        }
    }

    /// Decompress data.
    pub fn decompress(
        &mut self,
        compressed_data: &[u8],
        uncompressed_data: &mut [u8],
    ) -> Result<(), ErrorTrace> {
        if compressed_data.len() < 13 {
            return Err(keramics_core::error_trace_new!(
                "Invalid compressed data value too small"
            ));
        }
        keramics_core::debug_trace_data_and_structure!(
            "LzmaStreamHeader",
            0,
            &compressed_data[0..13],
            13,
            LzmaStreamHeader::debug_read_data(compressed_data)
        );
        let mut stream_header: LzmaStreamHeader = LzmaStreamHeader::new();

        match stream_header.read_data(compressed_data) {
            Ok(_) => {}
            Err(mut error) => {
                keramics_core::error_trace_add_frame!(error, "Unable to read stream header");
                return Err(error);
            }
        }
        if stream_header.uncompressed_data_size != LZMA_UNCOMPRESSED_SIZE_UNKNOWN
            && stream_header.uncompressed_data_size as usize > uncompressed_data.len()
        {
            return Err(keramics_core::error_trace_new!(
                "Invalid uncompressed data value too small"
            ));
        }
        let mut decoder: LzmaDecoder = LzmaDecoder::new(&stream_header);
        let compressed_stream: &[u8] = &compressed_data[13..];

        DebugTrace::scope(|debug_trace| {
            match decoder.decompress_stream(debug_trace, compressed_stream, uncompressed_data) {
                Ok(number_of_bytes) => {
                    self.uncompressed_data_size = number_of_bytes;
                }
                Err(mut error) => {
                    keramics_core::error_trace_add_frame!(error, "Unable to decompress data");
                    return Err(error);
                }
            }
            Ok(())
        })
    }
}

/// Decoder for decompressing LZMA compressed data.
struct LzmaDecoder {
    /// Number of literal context bits (lc).
    literal_context_bits: u8,

    /// Number of literal position bits (lp).
    literal_pos_bits: u8,

    /// Number of position bits (pb).
    pos_bits: u8,

    /// Uncompressed data size, or LZMA_UNCOMPRESSED_SIZE_UNKNOWN.
    uncompressed_data_size: u64,

    /// Decoder state (0 to 11).
    state: usize,

    /// Rep 0 distance (zero-based).
    rep0: usize,

    /// Rep 1 distance (zero-based).
    rep1: usize,

    /// Rep 2 distance (zero-based).
    rep2: usize,

    /// Rep 3 distance (zero-based).
    rep3: usize,

    /// Last decoded byte value, or zero if no bytes have been decoded yet.
    previous_byte: u8,

    /// Probabilities whether a literal or match is decoded next (state * 16 + position_state).
    is_match: Vec<u16>,

    /// Probabilities of a repeated match (state).
    is_rep: Vec<u16>,

    /// Probabilities that the repeated match distance is rep 0 (state).
    is_rep0: Vec<u16>,

    /// Probabilities that the repeated match distance is rep 1 (state).
    is_rep1: Vec<u16>,

    /// Probabilities that the repeated match distance is rep 2 (state).
    is_rep2: Vec<u16>,

    /// Probabilities of a long repeated match at rep 0 (state * 16 + position_state).
    is_rep0_long: Vec<u16>,

    /// Probabilities of the literal symbols (coder * 0x300 + subcoder).
    literal: Vec<u16>,

    /// Probabilities of the match length decoder.
    ///
    /// Consists of: choice (cell 0), choice2 (cell 1),
    /// low (cells 2 + position_state * 8), mid (cells 2 + 128 + position_state * 8)
    /// and high (cells 258).
    match_len: Vec<u16>,

    /// Probabilities of the rep match length decoder (same layout as match_len).
    rep_len: Vec<u16>,

    /// Probabilities of the distance slot (length state * 64 + slot).
    dist_slot: Vec<u16>,

    /// Probabilities of the distance low bits (position special decoders).
    dist_low: Vec<u16>,

    /// Probabilities of the distance align bits.
    dist_align: Vec<u16>,
}

impl LzmaDecoder {
    /// Creates a new decoder.
    pub fn new(stream_header: &LzmaStreamHeader) -> LzmaDecoder {
        let number_of_literal_states: usize =
            1 << (stream_header.literal_context_bits + stream_header.literal_pos_bits);

        Self {
            literal_context_bits: stream_header.literal_context_bits,
            literal_pos_bits: stream_header.literal_pos_bits,
            pos_bits: stream_header.pos_bits,
            uncompressed_data_size: stream_header.uncompressed_data_size,
            state: 0,
            rep0: 0,
            rep1: 0,
            rep2: 0,
            rep3: 0,
            previous_byte: 0,
            is_match: vec![LZMA_PROBABILITY_INIT; LZMA_NUMBER_OF_STATES * LZMA_POSITION_STATES],
            is_rep: vec![LZMA_PROBABILITY_INIT; LZMA_NUMBER_OF_STATES],
            is_rep0: vec![LZMA_PROBABILITY_INIT; LZMA_NUMBER_OF_STATES],
            is_rep1: vec![LZMA_PROBABILITY_INIT; LZMA_NUMBER_OF_STATES],
            is_rep2: vec![LZMA_PROBABILITY_INIT; LZMA_NUMBER_OF_STATES],
            is_rep0_long: vec![LZMA_PROBABILITY_INIT; LZMA_NUMBER_OF_STATES * LZMA_POSITION_STATES],
            literal: vec![LZMA_PROBABILITY_INIT; number_of_literal_states * 0x300],
            match_len: vec![LZMA_PROBABILITY_INIT; LZMA_MATCH_LENGTH_NUMBER_OF_CELLS],
            rep_len: vec![LZMA_PROBABILITY_INIT; LZMA_MATCH_LENGTH_NUMBER_OF_CELLS],
            dist_slot: vec![
                LZMA_PROBABILITY_INIT;
                LZMA_NUMBER_OF_DISTANCE_SLOT_STATES * LZMA_NUMBER_OF_DISTANCE_SLOTS
            ],
            // NOTE: 114 probability cells (indexes 0 to 113) are used by the
            // distance low bits (position special) decoders for slots 4 to 13.
            dist_low: vec![LZMA_PROBABILITY_INIT; 114],
            dist_align: vec![LZMA_PROBABILITY_INIT; 1 << LZMA_NUMBER_OF_DISTANCE_ALIGN_BITS],
        }
    }
}

impl LzmaDecoder {
    /// Updates the decoder state after a literal has been decoded.
    fn update_state_literal(&mut self) {
        let state: usize = self.state;

        self.state = if state <= 3 {
            0
        } else if state <= 9 {
            state - 3
        } else {
            state - 6
        };
    }

    /// Decompresses an LZMA stream and returns the number of decompressed bytes.
    fn decompress_stream(
        &mut self,
        debug_trace: &mut DebugTraceScope,
        compressed_data: &[u8],
        uncompressed_data: &mut [u8],
    ) -> Result<usize, ErrorTrace> {
        let mut range_decoder: LzmaRangeDecoder = LzmaRangeDecoder::new(compressed_data)?;
        let size_defined: bool = self.uncompressed_data_size != LZMA_UNCOMPRESSED_SIZE_UNKNOWN;
        let size: usize = if size_defined {
            self.uncompressed_data_size as usize
        } else {
            usize::MAX
        };
        let pos_mask: usize = (1 << self.pos_bits) - 1;
        let literal_pos_mask: usize = (1 << self.literal_pos_bits) - 1;

        // EOPM is always allowed in a standalone (LZMA1) stream with an unknown
        // uncompressed data size. When the uncompressed data size is known, EOPM
        // is only allowed if it occurs after exactly that number of bytes.
        let mut eopm_is_valid: bool = !size_defined;
        let mut offset: usize = 0;

        loop {
            if size_defined && offset >= size {
                range_decoder.normalize()?;

                if range_decoder.is_finished() {
                    break;
                }
                eopm_is_valid = true;
            }
            let pos_state: usize = offset & pos_mask;

            if lzma_decode_bit(
                &mut range_decoder,
                &mut self.is_match[self.state * LZMA_POSITION_STATES + pos_state],
            )? == 0
            {
                // Decode a literal.
                let subcoder: usize = (((offset & literal_pos_mask) << self.literal_context_bits)
                    + (self.previous_byte as usize >> (8 - self.literal_context_bits)))
                    * 0x300;
                let mut match_byte: u8 =
                    if self.state >= LZMA_NUMBER_OF_LITERAL_STATES && offset > self.rep0 {
                        uncompressed_data[offset - 1 - self.rep0]
                    } else {
                        0
                    };
                let mut matched: bool = self.state >= LZMA_NUMBER_OF_LITERAL_STATES;
                let mut symbol: usize = 1;

                while symbol < 0x100 {
                    let literal_bit: u32 = if matched {
                        // Use the subcoder for the matched byte bit.
                        //
                        // NOTE: the subcoder index of the matched literal cells is
                        // 0x100 + (match_bit * 0x100) + symbol.
                        let match_bit: u8 = (match_byte >> 7) & 1;
                        let bit: u32 = lzma_decode_bit(
                            &mut range_decoder,
                            &mut self.literal
                                [subcoder + ((1 + match_bit as usize) * 0x100) + symbol],
                        )?;
                        if bit != match_bit as u32 {
                            matched = false;
                        }
                        // NOTE: shift the match byte left, wrapping is fine (only the top 8 bits matter).
                        match_byte = match_byte.wrapping_shl(1);
                        bit
                    } else {
                        lzma_decode_bit(&mut range_decoder, &mut self.literal[subcoder + symbol])?
                    };
                    symbol = (symbol << 1) + literal_bit as usize;
                }
                let literal_value: u8 = (symbol & 0xFF) as u8;

                if size_defined && offset >= size {
                    return Err(keramics_core::error_trace_new!(
                        "Invalid uncompressed data value too small"
                    ));
                }
                if offset >= uncompressed_data.len() {
                    return Err(keramics_core::error_trace_new!(
                        "Invalid uncompressed data value too small"
                    ));
                }
                debug_trace.print_field("symbol", format!("literal: 0x{:02x}", literal_value));
                uncompressed_data[offset] = literal_value;
                offset += 1;
                self.previous_byte = literal_value;
                self.update_state_literal();
            } else {
                // Decode a match.
                if lzma_decode_bit(&mut range_decoder, &mut self.is_rep[self.state])? == 0 {
                    // Simple match.
                    let state: usize = self.state;
                    self.state = if state < LZMA_NUMBER_OF_LITERAL_STATES {
                        7
                    } else {
                        10
                    };
                    let match_length: usize =
                        lzma_decode_length(&mut range_decoder, pos_state, &mut self.match_len)?;
                    // Zero-based match length capped at 3 is used to select the distance slot table.
                    let len_state: usize = (match_length - LZMA_MATCH_LENGTH_MINIMUM).min(3);

                    let dist_slot_base: usize = len_state * LZMA_NUMBER_OF_DISTANCE_SLOTS;
                    let mut symbol: usize = 1;

                    while symbol < LZMA_NUMBER_OF_DISTANCE_SLOTS {
                        let bit: u32 = lzma_decode_bit(
                            &mut range_decoder,
                            &mut self.dist_slot[dist_slot_base + symbol],
                        )?;
                        symbol = (symbol << 1) + bit as usize;
                    }
                    let slot: usize = symbol - LZMA_NUMBER_OF_DISTANCE_SLOTS;
                    let distance: usize;

                    if slot < LZMA_DISTANCE_SLOT_LOW_BITS {
                        distance = slot;
                    } else if slot < LZMA_DISTANCE_SLOT_PLAIN_BITS {
                        // Decode the distance low bits using the position special decoders.
                        let number_of_bits: usize = (slot / 2) - 1;
                        let mut value: usize = (2 | (slot & 1)) << number_of_bits;
                        // NOTE: the first distance low bits cell index is (value - slot).
                        let table_base: usize = value - slot;
                        let mut table_symbol: usize = 1;
                        let mut bit_offset: usize = 0;

                        while bit_offset < number_of_bits {
                            let bit: u32 = lzma_decode_bit(
                                &mut range_decoder,
                                &mut self.dist_low[table_base + table_symbol - 1],
                            )?;
                            if bit == 1 {
                                value += 1 << bit_offset;
                            }
                            table_symbol = (table_symbol << 1) + bit as usize;
                            bit_offset += 1;
                        }
                        distance = value;
                    } else {
                        // Decode the distance middle bits as plain bits.
                        let number_of_bits: usize = (slot / 2) - 1;
                        let number_of_plain_bits: usize =
                            number_of_bits - LZMA_NUMBER_OF_DISTANCE_ALIGN_BITS;
                        let mut value: u32 = (2 | (slot & 1)) as u32;

                        for _ in 0..number_of_plain_bits {
                            value = (value << 1) | range_decoder.decode_direct_bit()?;
                        }
                        value <<= LZMA_NUMBER_OF_DISTANCE_ALIGN_BITS;
                        let mut table_symbol: usize = 1;
                        let mut bit_offset: usize = 0;

                        // Decode the distance low 4 bits using the align decoder.
                        while bit_offset < LZMA_NUMBER_OF_DISTANCE_ALIGN_BITS {
                            let bit: u32 = lzma_decode_bit(
                                &mut range_decoder,
                                &mut self.dist_align[table_symbol],
                            )?;
                            if bit == 1 {
                                value |= 1 << bit_offset;
                            }
                            table_symbol = (table_symbol << 1) + bit as usize;
                            bit_offset += 1;
                        }
                        if value == LZMA_END_OF_PAYLOAD_MARKER {
                            // End of payload marker (EOPM).
                            if !eopm_is_valid {
                                return Err(keramics_core::error_trace_new!(
                                    "Unexpected end of payload marker (EOPM)"
                                ));
                            }
                            range_decoder.normalize()?;

                            if !range_decoder.is_finished() {
                                return Err(keramics_core::error_trace_new!(
                                    "Invalid end of payload marker (EOPM)"
                                ));
                            }
                            debug_trace.print_field("symbol", String::from("EOPM"));
                            break;
                        }
                        distance = value as usize;
                    }
                    self.rep3 = self.rep2;
                    self.rep2 = self.rep1;
                    self.rep1 = self.rep0;
                    self.rep0 = distance;
                    if distance >= offset {
                        return Err(keramics_core::error_trace_new!(format!(
                            "Invalid match distance: {} out of bounds",
                            distance
                        )));
                    }
                    for _ in 0..match_length {
                        if size_defined && offset >= size {
                            return Err(keramics_core::error_trace_new!(
                                "Invalid uncompressed data value too small"
                            ));
                        }
                        if offset >= uncompressed_data.len() {
                            return Err(keramics_core::error_trace_new!(
                                "Invalid uncompressed data value too small"
                            ));
                        }
                        uncompressed_data[offset] = uncompressed_data[offset - 1 - distance];
                        offset += 1;
                        self.previous_byte = uncompressed_data[offset - 1];
                    }
                    debug_trace.print_field(
                        "symbol",
                        format!("match: length: {}, distance: {}", match_length, distance),
                    );
                } else {
                    // Repeated match.
                    if offset == 0 {
                        return Err(keramics_core::error_trace_new!("Invalid match at offset 0"));
                    }
                    let rep_distance: usize;

                    if lzma_decode_bit(&mut range_decoder, &mut self.is_rep0[self.state])? == 0 {
                        if lzma_decode_bit(
                            &mut range_decoder,
                            &mut self.is_rep0_long[self.state * LZMA_POSITION_STATES + pos_state],
                        )? == 0
                        {
                            // Short repeated match: repeat the last decoded byte at rep 0.
                            let state: usize = self.state;
                            self.state = if state < LZMA_NUMBER_OF_LITERAL_STATES {
                                9
                            } else {
                                11
                            };
                            rep_distance = self.rep0;
                            if size_defined && offset >= size {
                                return Err(keramics_core::error_trace_new!(
                                    "Invalid uncompressed data value too small"
                                ));
                            }
                            if offset >= uncompressed_data.len() {
                                return Err(keramics_core::error_trace_new!(
                                    "Invalid uncompressed data value too small"
                                ));
                            }
                            // NOTE: a short repeated match must be a valid distance.
                            if rep_distance >= offset {
                                return Err(keramics_core::error_trace_new!(format!(
                                    "Invalid match distance: {} out of bounds",
                                    rep_distance
                                )));
                            }
                            uncompressed_data[offset] =
                                uncompressed_data[offset - 1 - rep_distance];
                            offset += 1;
                            self.previous_byte = uncompressed_data[offset - 1];
                            debug_trace.print_field(
                                "symbol",
                                format!("short rep: distance: {}", rep_distance),
                            );
                            continue;
                        }
                        // Long repeated match at rep 0.
                        rep_distance = self.rep0;
                    } else if lzma_decode_bit(&mut range_decoder, &mut self.is_rep1[self.state])?
                        == 0
                    {
                        // NOTE: rep 1 becomes rep 0.
                        let distance: usize = self.rep1;

                        self.rep1 = self.rep0;
                        self.rep0 = distance;
                        rep_distance = self.rep0;
                    } else if lzma_decode_bit(&mut range_decoder, &mut self.is_rep2[self.state])?
                        == 0
                    {
                        // NOTE: rep 2 becomes rep 0.
                        let distance: usize = self.rep2;

                        self.rep2 = self.rep1;
                        self.rep1 = self.rep0;
                        self.rep0 = distance;
                        rep_distance = self.rep0;
                    } else {
                        // NOTE: rep 3 becomes rep 0.
                        let distance: usize = self.rep3;

                        self.rep3 = self.rep2;
                        self.rep2 = self.rep1;
                        self.rep1 = self.rep0;
                        self.rep0 = distance;
                        rep_distance = self.rep0;
                    }
                    let state: usize = self.state;
                    self.state = if state < LZMA_NUMBER_OF_LITERAL_STATES {
                        8
                    } else {
                        11
                    };
                    let match_length: usize =
                        lzma_decode_length(&mut range_decoder, pos_state, &mut self.rep_len)?;
                    for _ in 0..match_length {
                        if size_defined && offset >= size {
                            return Err(keramics_core::error_trace_new!(
                                "Invalid uncompressed data value too small"
                            ));
                        }
                        if offset >= uncompressed_data.len() {
                            return Err(keramics_core::error_trace_new!(
                                "Invalid uncompressed data value too small"
                            ));
                        }
                        // NOTE: the rep distance must have been a valid distance when it was stored.
                        if rep_distance >= offset {
                            return Err(keramics_core::error_trace_new!(format!(
                                "Invalid match distance: {} out of bounds",
                                rep_distance
                            )));
                        }
                        uncompressed_data[offset] = uncompressed_data[offset - 1 - rep_distance];
                        offset += 1;
                        self.previous_byte = uncompressed_data[offset - 1];
                    }
                    debug_trace.print_field(
                        "symbol",
                        format!(
                            "long rep: length: {}, distance: {}",
                            match_length, rep_distance
                        ),
                    );
                }
            }
        }
        Ok(offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LZMA stream of "hello world" repeated 200 times (2200 bytes).
    ///
    /// Uses an unknown uncompressed data size and an end of payload marker (EOPM).
    const TEST_DATA_P1_ALONE: [u8; 49] = [
        0x5d, 0x00, 0x00, 0x10, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 0x34,
        0x19, 0x49, 0xdb, 0x85, 0x5c, 0x63, 0xad, 0x3e, 0xf9, 0x63, 0x76, 0x52, 0x91, 0xc1, 0x28,
        0xe5, 0xe3, 0x72, 0x09, 0x59, 0xe6, 0x44, 0x88, 0x1d, 0x01, 0x63, 0x02, 0x47, 0xff, 0xff,
        0xf4, 0xd4, 0x80, 0x00,
    ];

    /// LZMA stream of "hello world" repeated 200 times (2200 bytes).
    ///
    /// Uses a known uncompressed data size and an end of payload marker (EOPM).
    const TEST_DATA_P1_KNOWN_SIZE_EOPM: [u8; 49] = [
        0x5d, 0x00, 0x00, 0x10, 0x00, 0x98, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x34,
        0x19, 0x49, 0xdb, 0x85, 0x5c, 0x63, 0xad, 0x3e, 0xf9, 0x63, 0x76, 0x52, 0x91, 0xc1, 0x28,
        0xe5, 0xe3, 0x72, 0x09, 0x59, 0xe6, 0x44, 0x88, 0x1d, 0x01, 0x63, 0x02, 0x47, 0xff, 0xff,
        0xf4, 0xd4, 0x80, 0x00,
    ];

    /// LZMA stream of "hello world" repeated 200 times (2200 bytes).
    ///
    /// Uses a known uncompressed data size and no end of payload marker (EOPM).
    const TEST_DATA_P1_KNOWN_SIZE_NO_EOPM: [u8; 49] = [
        0x5d, 0x00, 0x00, 0x80, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 0x34,
        0x19, 0x49, 0xee, 0x8d, 0xe9, 0x17, 0x89, 0x3a, 0x33, 0x60, 0x0a, 0x36, 0x3d, 0xd0, 0xbf,
        0x6e, 0xfc, 0xeb, 0xa2, 0x43, 0x50, 0x04, 0xde, 0x91, 0x7b, 0x7b, 0x3d, 0x95, 0xff, 0xff,
        0xc1, 0xf4, 0x00, 0x00,
    ];

    /// LZMA stream of bytes 0x00 to 0xff repeated 16 times (4096 bytes).
    const TEST_DATA_PATTERN_4096: [u8; 277] = [
        0x5d, 0x00, 0x00, 0x80, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00,
        0x00, 0x52, 0x50, 0x0a, 0x84, 0xf9, 0x9b, 0xb2, 0x80, 0x21, 0xa9, 0x69, 0xd6, 0x27, 0xe0,
        0x3e, 0x06, 0x5a, 0x5f, 0x04, 0x8d, 0x53, 0xd4, 0x04, 0xba, 0x39, 0x57, 0x05, 0x09, 0xc1,
        0x55, 0x24, 0xde, 0x9d, 0xb8, 0x71, 0x59, 0x31, 0x60, 0xa1, 0x9f, 0xf9, 0x6f, 0x49, 0x73,
        0xf2, 0xc8, 0xea, 0x8c, 0xba, 0x1a, 0x8b, 0x29, 0x69, 0x21, 0x80, 0xfe, 0x33, 0x83, 0x66,
        0xaf, 0x46, 0x6d, 0xec, 0x9e, 0x89, 0x8a, 0x0b, 0x83, 0xf0, 0x3c, 0x0e, 0x89, 0x8e, 0x3f,
        0xed, 0x5f, 0xe7, 0x9e, 0x90, 0xd9, 0x1c, 0xff, 0x32, 0xf4, 0xb2, 0xe0, 0x39, 0x51, 0xb2,
        0xd2, 0x14, 0x15, 0xb4, 0xc5, 0x71, 0xba, 0xdb, 0x06, 0xe3, 0x79, 0x9a, 0x9f, 0xbb, 0x38,
        0xc1, 0xb0, 0x00, 0xac, 0x93, 0x0b, 0xaa, 0x06, 0x19, 0x03, 0x12, 0x08, 0x15, 0x5b, 0x9b,
        0xc8, 0x48, 0xf0, 0x32, 0x2e, 0xfe, 0x2d, 0xa0, 0x87, 0xc8, 0xf0, 0xa4, 0xe0, 0xd2, 0x51,
        0xeb, 0x8d, 0x67, 0x56, 0x92, 0xb2, 0x4d, 0x84, 0xc5, 0xf1, 0x86, 0x31, 0xdf, 0x6a, 0x62,
        0x5b, 0xc2, 0x79, 0x2d, 0xd9, 0xf7, 0x3c, 0x73, 0xba, 0x74, 0x74, 0x07, 0xd8, 0x3c, 0xa9,
        0x56, 0x22, 0x24, 0xa1, 0x66, 0xf8, 0x5a, 0x84, 0x5f, 0x30, 0x67, 0xd2, 0xf6, 0x4b, 0x49,
        0x2e, 0x7f, 0x20, 0xeb, 0xdb, 0xf8, 0x10, 0x0e, 0x94, 0x78, 0x77, 0xc7, 0x3f, 0x6b, 0xef,
        0xb4, 0xcd, 0x95, 0xe2, 0x6f, 0xf6, 0x44, 0x6e, 0x06, 0xcf, 0x0b, 0x82, 0x1a, 0xcb, 0xdb,
        0x7a, 0xf0, 0x57, 0x8d, 0x98, 0xff, 0x90, 0xc0, 0x3e, 0xe6, 0xc1, 0x12, 0x41, 0x75, 0xee,
        0x03, 0x28, 0x96, 0xeb, 0x13, 0xfb, 0xa7, 0x28, 0xcc, 0xaf, 0x32, 0xbb, 0xa4, 0x0e, 0x25,
        0xf2, 0x58, 0xb0, 0xde, 0xd8, 0x56, 0x1c, 0x66, 0xf0, 0xe2, 0x1b, 0x35, 0x5e, 0xdb, 0x2c,
        0x46, 0xff, 0xff, 0x76, 0x21, 0x40, 0x00,
    ];

    /// LZMA stream of data that uses simple matches at each distance slot 4 to 13.
    const TEST_DATA_DISTANCE_SLOTS: [u8; 662] = [
        0x5d, 0x00, 0x00, 0x80, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 0x6f,
        0xbe, 0xd6, 0xaf, 0xbf, 0xdd, 0x0d, 0xc9, 0xb9, 0x18, 0x3d, 0x79, 0x74, 0xdf, 0xa6, 0xf5,
        0x45, 0x65, 0x19, 0xbb, 0x24, 0x47, 0x09, 0x7e, 0xfc, 0xa8, 0xe8, 0x62, 0x70, 0x39, 0x65,
        0x56, 0x75, 0xbd, 0x21, 0x71, 0xe2, 0x5b, 0x84, 0x1f, 0xc9, 0xff, 0x8c, 0x9a, 0x2a, 0x26,
        0xb7, 0x77, 0x3e, 0x48, 0xa0, 0x8a, 0xaf, 0x4f, 0x12, 0x6f, 0x6c, 0x74, 0x19, 0x91, 0x72,
        0xce, 0x4d, 0x3b, 0x54, 0x42, 0x2a, 0x52, 0xdd, 0xc8, 0x95, 0xbf, 0xd8, 0x74, 0x2f, 0x81,
        0xc3, 0x7b, 0xad, 0x38, 0x11, 0xcc, 0x42, 0x01, 0xfb, 0x89, 0x84, 0xbb, 0xb0, 0x78, 0xfd,
        0x8c, 0x04, 0x9e, 0xc3, 0x90, 0x2d, 0x8c, 0x11, 0x16, 0x10, 0x30, 0x33, 0xdb, 0x61, 0xd6,
        0x2b, 0x3b, 0xf6, 0x47, 0x19, 0x22, 0xcd, 0x41, 0xc1, 0x62, 0xdb, 0xdb, 0x54, 0x52, 0xe6,
        0x13, 0x39, 0x20, 0x0f, 0x88, 0x26, 0x6b, 0x07, 0x09, 0x1e, 0x72, 0x67, 0x4b, 0x43, 0xe1,
        0xea, 0xd0, 0x74, 0xdc, 0xbe, 0x0f, 0x1e, 0x9c, 0x4f, 0x75, 0x8e, 0x8f, 0x23, 0x0d, 0x43,
        0xe3, 0x35, 0x3f, 0x97, 0x5e, 0x79, 0x54, 0x92, 0x79, 0x64, 0x04, 0xd3, 0xa8, 0xdf, 0xf9,
        0xb7, 0x8a, 0x6b, 0x62, 0x9d, 0x55, 0x48, 0x1b, 0xa2, 0x33, 0x84, 0x68, 0xd6, 0x0a, 0x5c,
        0x39, 0x07, 0xa9, 0x18, 0x76, 0x22, 0x25, 0x09, 0x44, 0x45, 0x9c, 0x48, 0x70, 0x1f, 0x56,
        0xee, 0xd5, 0x87, 0xb4, 0x89, 0xcf, 0xec, 0x7b, 0xc3, 0x6b, 0xc2, 0xeb, 0xee, 0xbe, 0x61,
        0x8b, 0x5e, 0xe1, 0x28, 0xb6, 0x55, 0x47, 0xb8, 0x2e, 0x73, 0x22, 0x6a, 0xde, 0xc9, 0x21,
        0x24, 0x2c, 0x7c, 0x85, 0x21, 0x14, 0x88, 0x75, 0xa7, 0xd2, 0x67, 0xa8, 0x0f, 0x52, 0x30,
        0x8f, 0x9f, 0x9a, 0x3d, 0x0d, 0xbf, 0xe9, 0xfc, 0x34, 0x50, 0x75, 0x9b, 0xb0, 0x0f, 0xea,
        0xa9, 0x46, 0x14, 0x05, 0x53, 0xdb, 0xf3, 0x0d, 0x44, 0x8a, 0x6e, 0x54, 0x42, 0x04, 0xb8,
        0x35, 0x20, 0x96, 0x58, 0xef, 0xbd, 0x97, 0xe5, 0x91, 0xb5, 0xa3, 0xfe, 0x54, 0xd3, 0xba,
        0x78, 0x7d, 0xf3, 0x42, 0x72, 0xe9, 0x1a, 0x3e, 0x27, 0x3e, 0x69, 0xe0, 0x0c, 0x6b, 0x6c,
        0xc5, 0x23, 0x91, 0xd6, 0xfe, 0x2c, 0x8d, 0x11, 0xa6, 0xf2, 0xb4, 0x68, 0x6e, 0x5d, 0x58,
        0x99, 0x43, 0x52, 0xb8, 0x63, 0xf2, 0x26, 0xc1, 0xff, 0x7b, 0xcf, 0x5a, 0xff, 0xde, 0x4a,
        0x40, 0x26, 0xc2, 0xa4, 0x5e, 0x2a, 0x85, 0xaa, 0x47, 0xb7, 0xc9, 0x1e, 0x4c, 0xf6, 0x78,
        0xda, 0x33, 0x38, 0x8f, 0xa6, 0x07, 0x61, 0xd4, 0xae, 0x52, 0x47, 0xb3, 0x8f, 0xd2, 0x75,
        0x1f, 0xd6, 0x61, 0x9f, 0xb9, 0x07, 0xc5, 0xaa, 0xdc, 0x94, 0xbf, 0xdb, 0x4e, 0xa4, 0x20,
        0x41, 0xf0, 0x37, 0xa8, 0x66, 0xa3, 0x27, 0x6d, 0xac, 0x22, 0xea, 0x41, 0xeb, 0x35, 0x66,
        0x88, 0xe0, 0x3a, 0xb5, 0x9f, 0xd7, 0xc5, 0x41, 0xd3, 0x77, 0x6c, 0x2c, 0x75, 0x77, 0x53,
        0x8c, 0x41, 0x45, 0x18, 0x03, 0x46, 0x8d, 0x9b, 0x74, 0x6f, 0x61, 0x3b, 0x35, 0xf5, 0xfe,
        0x40, 0x16, 0x25, 0xa9, 0x0f, 0x6d, 0x60, 0x50, 0xc7, 0x07, 0xba, 0x96, 0x9d, 0xae, 0xc7,
        0x8b, 0x39, 0xb0, 0x46, 0x7b, 0xb9, 0xf9, 0xe8, 0xf5, 0x33, 0x04, 0x31, 0x9e, 0xb6, 0x9f,
        0x47, 0x59, 0x85, 0x7f, 0x8c, 0x06, 0x1e, 0x71, 0xf3, 0xe2, 0xea, 0x54, 0x0c, 0x2d, 0x9c,
        0x42, 0xd2, 0x22, 0xb0, 0x9c, 0xff, 0x95, 0xac, 0x18, 0x80, 0xfb, 0xcb, 0xae, 0xf8, 0xf3,
        0xf6, 0x7b, 0x6b, 0xc0, 0x5d, 0xc0, 0xa3, 0x65, 0x4c, 0x30, 0x49, 0x78, 0x17, 0xd5, 0xe1,
        0xff, 0x2b, 0x25, 0xec, 0x62, 0xe3, 0x67, 0x8b, 0x10, 0x24, 0x42, 0x5c, 0x92, 0xde, 0x55,
        0xb4, 0x04, 0xb9, 0xbe, 0xd1, 0xa2, 0xb5, 0x95, 0x5d, 0x8b, 0x90, 0xb7, 0x72, 0x9b, 0x41,
        0xe9, 0x16, 0x44, 0x11, 0x1c, 0x43, 0x7f, 0x00, 0x13, 0xe8, 0xdb, 0x83, 0xde, 0x36, 0xfc,
        0x81, 0xc5, 0x90, 0x9c, 0x73, 0x4f, 0x3f, 0x8a, 0xc8, 0x79, 0x81, 0x44, 0x11, 0xc5, 0x35,
        0x73, 0x33, 0x68, 0x77, 0x11, 0x5a, 0x66, 0x63, 0x80, 0x26, 0xd2, 0x6c, 0x00, 0x13, 0xbb,
        0xcd, 0x5f, 0x86, 0xc9, 0xfe, 0x73, 0x6e, 0x25, 0xe1, 0x2a, 0xb5, 0xc9, 0x4e, 0x05, 0x39,
        0x57, 0x95, 0xab, 0x4d, 0xb0, 0xe8, 0x90, 0x2b, 0x96, 0xb6, 0xeb, 0x8a, 0x17, 0xe9, 0x2d,
        0xa0, 0x5e, 0x03, 0x02, 0x69, 0xff, 0x49, 0x40, 0x14, 0x5b, 0x72, 0x84, 0x27, 0x76, 0xe9,
        0x48, 0x4e, 0x47, 0x88, 0x2f, 0xa7, 0x76, 0xf3, 0x5a, 0x7d, 0x3d, 0xe6, 0x98, 0xff, 0xa9,
        0xd4, 0xb5, 0x62, 0x85, 0xee, 0xad, 0x6d, 0xf1, 0x23, 0xdc, 0x52, 0xff, 0xff, 0xb3, 0x92,
        0x00, 0x00,
    ];

    /// LZMA stream of data that uses small match distances 1 to 300.
    const TEST_DATA_SMALL_DISTANCES: [u8; 355] = [
        0x5d, 0x00, 0x00, 0x80, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00,
        0xe0, 0x07, 0xbf, 0xda, 0x3e, 0x02, 0x6b, 0x9c, 0x30, 0xd9, 0x73, 0x37, 0xdd, 0x1c, 0x3d,
        0x12, 0x7f, 0x70, 0x50, 0x74, 0x08, 0xf9, 0x81, 0x74, 0xee, 0x26, 0x17, 0xa4, 0x7a, 0xff,
        0xf0, 0x84, 0xa6, 0x59, 0xa4, 0x4a, 0x7d, 0x40, 0x95, 0xa5, 0x79, 0xd2, 0xb9, 0x4a, 0x0f,
        0xf5, 0xf2, 0xf3, 0x5e, 0x59, 0xf8, 0xda, 0x9d, 0x45, 0x12, 0x75, 0x57, 0x28, 0x6f, 0x4b,
        0x47, 0x71, 0x4b, 0x19, 0x27, 0xbf, 0x91, 0x4b, 0x68, 0x9b, 0x4f, 0xd8, 0x94, 0xba, 0x42,
        0x6a, 0x12, 0xfd, 0x53, 0x38, 0x16, 0x2c, 0x3e, 0xa5, 0xe7, 0x2c, 0x10, 0xe3, 0x29, 0xda,
        0xde, 0xeb, 0x59, 0x78, 0x51, 0x5b, 0xa7, 0xce, 0xd7, 0x57, 0x76, 0xd3, 0xad, 0x2a, 0x36,
        0xa6, 0x0a, 0x4c, 0x48, 0x8b, 0xfa, 0x01, 0x83, 0x27, 0xa1, 0x49, 0x1a, 0xda, 0x92, 0x5b,
        0x57, 0x14, 0x80, 0x25, 0x9e, 0x7b, 0x18, 0x3a, 0xf3, 0x27, 0x95, 0x09, 0x2f, 0xc4, 0x4f,
        0x35, 0xba, 0x50, 0x68, 0x77, 0xa1, 0x03, 0xb7, 0xef, 0x1e, 0xc7, 0xa4, 0x71, 0x68, 0x47,
        0x78, 0x9d, 0x40, 0x06, 0x2f, 0xb0, 0xa6, 0xe7, 0xd8, 0xab, 0x8f, 0x7f, 0x7f, 0x14, 0x19,
        0x3f, 0xf0, 0x77, 0x39, 0x0c, 0xed, 0xcd, 0x30, 0xab, 0x4b, 0x55, 0x66, 0x6b, 0x60, 0x42,
        0xd1, 0x98, 0xd5, 0x16, 0xe6, 0x8b, 0x28, 0xb6, 0xe4, 0xcc, 0x09, 0xc7, 0x43, 0x2c, 0xf4,
        0x37, 0xc9, 0x0d, 0x50, 0x62, 0x27, 0x7d, 0x86, 0xfe, 0xdf, 0xa3, 0x15, 0x53, 0xe5, 0x2e,
        0xb6, 0xc4, 0x24, 0xab, 0x82, 0x99, 0xbf, 0x4e, 0x38, 0xa9, 0xc5, 0x07, 0xc7, 0xb9, 0xfb,
        0xd5, 0x7b, 0x86, 0xee, 0x87, 0x97, 0x38, 0xa8, 0x3c, 0xbb, 0xf2, 0x6f, 0x19, 0xf2, 0x32,
        0xbd, 0xb1, 0x5c, 0xe9, 0x28, 0xa7, 0x90, 0xdc, 0xb9, 0x66, 0xcc, 0x61, 0x87, 0xe8, 0x0c,
        0x69, 0xcf, 0x8e, 0x40, 0x31, 0x86, 0x7f, 0x99, 0x3a, 0x2f, 0xc9, 0xeb, 0x54, 0x4e, 0xf4,
        0xca, 0x86, 0xc1, 0xf8, 0x30, 0xf8, 0x50, 0xcf, 0x20, 0x35, 0xa3, 0x7b, 0x13, 0xe5, 0xbd,
        0x20, 0x35, 0xdf, 0xb4, 0x19, 0x0b, 0xa3, 0x8a, 0x1e, 0x59, 0x39, 0xc6, 0x1a, 0xe2, 0x5b,
        0x36, 0x56, 0x58, 0x75, 0x91, 0xc9, 0xab, 0x35, 0xe5, 0x9c, 0x20, 0xd3, 0x7b, 0xe9, 0x88,
        0xc6, 0x31, 0x2b, 0x5d, 0x0f, 0xf7, 0x82, 0x5e, 0x4d, 0x6a, 0x75, 0x3d, 0xff, 0x5f, 0xe1,
        0x99, 0xce, 0x49, 0xb5, 0x4c, 0xff, 0xfe, 0x13, 0xc8, 0x00,
    ];

    /// LZMA stream of 5000 zero bytes that uses repeated matches.
    const TEST_DATA_ZEROS: [u8; 49] = [
        0x5d, 0x00, 0x00, 0x80, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00,
        0x6f, 0xfd, 0xff, 0xff, 0xa3, 0xb7, 0xff, 0x47, 0x3e, 0x48, 0x15, 0x72, 0x39, 0x61, 0x51,
        0xb8, 0x92, 0x28, 0xe6, 0xa3, 0x86, 0x07, 0xf9, 0xee, 0xcd, 0xd4, 0xd4, 0x26, 0x1f, 0xff,
        0xfd, 0x84, 0x40, 0x00,
    ];

    /// Generates a pseudo-random data sequence like the LZMA test data generators did.
    fn pseudo_random(number_of_bytes: usize, mut seed: u64) -> Vec<u8> {
        let mut data: Vec<u8> = vec![];

        while data.len() < number_of_bytes {
            seed = (seed.wrapping_mul(1103515245).wrapping_add(12345)) & 0x7fffffff;
            data.push(((seed >> 16) % 251 + 3) as u8);
        }
        data
    }

    /// Retrievs the expected data for TEST_DATA_DISTANCE_SLOTS.
    fn get_test_data_distance_slots() -> Vec<u8> {
        let distances: [usize; 13] = [5, 6, 7, 9, 13, 18, 30, 36, 52, 65, 90, 102, 127];
        let mut data: Vec<u8> = vec![];

        for distance in distances {
            let block_size: usize = distance - 1;
            let block: Vec<u8> = pseudo_random(block_size, (7 * distance + 13) as u64);

            data.extend_from_slice(&block);
            data.push(220 + (distance % 30) as u8);
            data.extend_from_slice(&block);
            data.push(2);
            data.push(200 + (distance % 50) as u8);
        }
        data
    }

    #[test]
    fn test_read_stream_header() -> Result<(), ErrorTrace> {
        let mut stream_header: LzmaStreamHeader = LzmaStreamHeader::new();

        stream_header.read_data(&TEST_DATA_P1_ALONE)?;
        assert_eq!(stream_header.literal_context_bits, 3);
        assert_eq!(stream_header.literal_pos_bits, 0);
        assert_eq!(stream_header.pos_bits, 2);
        assert_eq!(stream_header.dictionary_size, 1048576);
        assert_eq!(stream_header.uncompressed_data_size, u64::MAX);

        Ok(())
    }

    #[test]
    fn test_read_stream_header_with_invalid_size() {
        let mut stream_header: LzmaStreamHeader = LzmaStreamHeader::new();

        let test_data: [u8; 12] = [
            0x5d, 0x00, 0x00, 0x10, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        ];
        let result: Result<(), ErrorTrace> = stream_header.read_data(&test_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_stream_header_with_invalid_properties() {
        let mut stream_header: LzmaStreamHeader = LzmaStreamHeader::new();

        let mut test_data: [u8; 13] = [
            0x5d, 0x00, 0x00, 0x10, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        ];
        test_data[0] = 0xE0;
        let result: Result<(), ErrorTrace> = stream_header.read_data(&test_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_decompress() -> Result<(), ErrorTrace> {
        let mut test_context: LzmaContext = LzmaContext::new();

        let expected_data: Vec<u8> = b"hello world".to_vec().repeat(200);
        let mut uncompressed_data: Vec<u8> = vec![0u8; expected_data.len()];
        test_context.decompress(&TEST_DATA_P1_ALONE, &mut uncompressed_data)?;
        assert_eq!(test_context.uncompressed_data_size, expected_data.len());
        assert_eq!(&uncompressed_data, &expected_data);

        Ok(())
    }

    #[test]
    fn test_decompress_with_known_size_and_eopm() -> Result<(), ErrorTrace> {
        let mut test_context: LzmaContext = LzmaContext::new();

        let expected_data: Vec<u8> = b"hello world".to_vec().repeat(200);
        let mut uncompressed_data: Vec<u8> = vec![0u8; expected_data.len()];
        test_context.decompress(&TEST_DATA_P1_KNOWN_SIZE_EOPM, &mut uncompressed_data)?;
        assert_eq!(test_context.uncompressed_data_size, expected_data.len());
        assert_eq!(&uncompressed_data, &expected_data);

        Ok(())
    }

    #[test]
    fn test_decompress_with_known_size_and_no_eopm() -> Result<(), ErrorTrace> {
        let mut test_context: LzmaContext = LzmaContext::new();

        let expected_data: Vec<u8> = b"hello world".to_vec().repeat(200);
        let mut uncompressed_data: Vec<u8> = vec![0u8; expected_data.len()];
        test_context.decompress(&TEST_DATA_P1_KNOWN_SIZE_NO_EOPM, &mut uncompressed_data)?;
        assert_eq!(test_context.uncompressed_data_size, expected_data.len());
        assert_eq!(&uncompressed_data, &expected_data);

        Ok(())
    }

    #[test]
    fn test_decompress_with_pattern_4096() -> Result<(), ErrorTrace> {
        let mut test_context: LzmaContext = LzmaContext::new();

        let mut expected_data: Vec<u8> = vec![];
        for _ in 0..16 {
            expected_data.extend_from_slice(&(0..=255u8).collect::<Vec<u8>>());
        }
        let mut uncompressed_data: Vec<u8> = vec![0u8; expected_data.len()];
        test_context.decompress(&TEST_DATA_PATTERN_4096, &mut uncompressed_data)?;
        assert_eq!(test_context.uncompressed_data_size, expected_data.len());
        assert_eq!(&uncompressed_data, &expected_data);

        Ok(())
    }

    #[test]
    fn test_decompress_with_distance_slots() -> Result<(), ErrorTrace> {
        let mut test_context: LzmaContext = LzmaContext::new();

        let expected_data: Vec<u8> = get_test_data_distance_slots();
        let mut uncompressed_data: Vec<u8> = vec![0u8; expected_data.len()];
        test_context.decompress(&TEST_DATA_DISTANCE_SLOTS, &mut uncompressed_data)?;
        assert_eq!(test_context.uncompressed_data_size, expected_data.len());
        assert_eq!(&uncompressed_data, &expected_data);

        Ok(())
    }

    #[test]
    fn test_decompress_with_small_distances() -> Result<(), ErrorTrace> {
        let mut test_context: LzmaContext = LzmaContext::new();

        let mut expected_data: Vec<u8> = vec![];
        for value in 1..300 {
            let byte: u8 = (value % 251) as u8;

            expected_data.extend(std::iter::repeat(byte).take(value % 3 + 1));
        }
        let mut uncompressed_data: Vec<u8> = vec![0u8; expected_data.len()];
        test_context.decompress(&TEST_DATA_SMALL_DISTANCES, &mut uncompressed_data)?;
        assert_eq!(test_context.uncompressed_data_size, expected_data.len());
        assert_eq!(&uncompressed_data, &expected_data);

        Ok(())
    }

    #[test]
    fn test_decompress_with_repeated_matches() -> Result<(), ErrorTrace> {
        let mut test_context: LzmaContext = LzmaContext::new();

        let expected_data: Vec<u8> = vec![0u8; 5000];
        let mut uncompressed_data: Vec<u8> = vec![0u8; expected_data.len()];
        test_context.decompress(&TEST_DATA_ZEROS, &mut uncompressed_data)?;
        assert_eq!(test_context.uncompressed_data_size, expected_data.len());
        assert_eq!(&uncompressed_data, &expected_data);

        Ok(())
    }

    #[test]
    fn test_decompress_with_insufficient_compressed_data() {
        let mut test_context: LzmaContext = LzmaContext::new();

        let test_data: [u8; 12] = [
            0x5d, 0x00, 0x00, 0x10, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        ];
        let mut uncompressed_data: Vec<u8> = vec![0u8; 2200];
        let result: Result<(), ErrorTrace> =
            test_context.decompress(&test_data, &mut uncompressed_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_decompress_with_invalid_range_coder_init_byte() {
        let mut test_context: LzmaContext = LzmaContext::new();

        let mut test_data: Vec<u8> = TEST_DATA_P1_ALONE.to_vec();

        test_data[13] = 0x01;
        let mut uncompressed_data: Vec<u8> = vec![0u8; 2200];
        let result: Result<(), ErrorTrace> =
            test_context.decompress(&test_data, &mut uncompressed_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_decompress_with_unsupported_size() {
        let mut test_context: LzmaContext = LzmaContext::new();

        let mut uncompressed_data: Vec<u8> = vec![0u8; 100];
        let result: Result<(), ErrorTrace> =
            test_context.decompress(&TEST_DATA_P1_KNOWN_SIZE_EOPM, &mut uncompressed_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_decompress_with_truncated_stream() {
        let mut test_context: LzmaContext = LzmaContext::new();

        let mut uncompressed_data: Vec<u8> = vec![0u8; 2200];
        let result: Result<(), ErrorTrace> = test_context.decompress(
            &TEST_DATA_P1_ALONE[0..TEST_DATA_P1_ALONE.len() - 2],
            &mut uncompressed_data,
        );
        assert!(result.is_err());
    }
}
