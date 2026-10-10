#!/usr/bin/env python3
# Usage:
#   python3 scripts/check_doc_offset_tables.py [PATH ...]
#
# Validates the "Offset | Size | ..." tables in the documentation (default:
# docs/formats/src/*.md):
#   * within a table segment, offsets must be in ascending order
#   * within a table segment, an offset must equal the previous offset plus the
#     previous (fixed) size
#   * bit tables ("| 0.0 | 1 bit |") must be contiguous within a segment
#   * a total-size statement directly above the table (e.g. "is 32 bytes in
#     size and consists of") must match the sum of the table rows (only when
#     the table is a single contiguous run starting at offset 0)
#
# Table conventions handled:
#   * annotation rows ("*If ...*", colspan rows) act as *separators*: the
#     offset run restarts in each segment and segments are not compared to each
#     other (they may describe alternative layouts)
#   * variable sizes ("...", "N x number of entries", "(size)") terminate the
#     verifiable run of a segment
#   * both decimal and 0x hexadecimal offsets, "N x M" sizes, and bold values
#   * bit offsets written as "byte.bit"

import re
import sys

from collections import namedtuple
from pathlib import Path

DEFAULT_PATHS = ["docs/formats/src"]

# A single data row from an offset table. Annotation rows leave offset_cell and
# size_cell empty and set is_annotation.
TableRow = namedtuple(
    "TableRow", ["line_number", "offset_cell", "size_cell", "is_annotation"]
)
# A parsed, verifiable row: offset and size in the table's unit.
TableRowValue = namedtuple("TableRowValue", ["line_number", "offset", "size"])


class OffsetTable:
    """Class that represents a table containing offset and size columns."""

    def __init__(self, file_path, start_line):
        """Creates an offset table."""
        super().__init__()
        self.file_path = file_path
        self.start_line = start_line
        self.rows = []

    def add_annotation_row(self, line_number):
        """Adds an annotation row."""
        table_row = TableRow(line_number, None, None, True)
        self.rows.append(table_row)

    def add_row(self, line_number, cells):
        """Adds a row."""
        table_row = TableRow(line_number, cells[0], cells[1], False)
        self.rows.append(table_row)


class OffsetTableValidator:
    """Parses and validates offset/size tables, collecting issues."""

    BYTE_UNIT = "byte"
    BIT_UNIT = "bit"

    BIT_SIZE_PATTERN = re.compile(r"^(\d+|\.\.\.)\s*(?:bit|bits)$")
    NUMBER_PATTERN = re.compile(r"^(\d+)$")
    PRODUCT_PATTERN = re.compile(r"^(\d+)\s*x\s*(\d+)(?:\s*=\s*(\d+))?$")
    BIT_OFFSET_PATTERN = re.compile(r"^(\d+)(?:\.(\d+))?$")
    OFFSET_VALUE_PATTERN = re.compile(r"^(\d+|\b0x[0-9a-fA-F]+)")
    TOTAL_SIZE_PATTERN = re.compile(r"\b(\d[\d_ ,]*)\s*(?:bytes|byte)\b")

    def __init__(self):
        """Creates an offset table validator."""
        super().__init__()
        self.issues = []

    def _check_contiguity(self, table, contiguous_rows, unit):
        for index in range(1, len(contiguous_rows)):
            previous_row = contiguous_rows[index - 1]
            current_row = contiguous_rows[index]
            expected_offset = previous_row.offset + previous_row.size
            if current_row.offset != expected_offset:
                if current_row.offset < previous_row.offset:
                    detail = (
                        f"offset {current_row.offset:d} is less than previous "
                        f"offset {previous_row.offset:d}"
                    )
                else:
                    detail = (
                        f"offset {current_row.offset:d} != previous offset "
                        f"{previous_row.offset:d} + previous size {previous_row.size:d} "
                        f"(= {expected_offset:d})"
                    )
                self.issues.append(
                    f"{table.file_path}:{current_row.line_number:d}: "
                    f"{detail} ({unit} table)"
                )

    def _check_total_size(self, table, lines, unit, contiguous_rows, segment):
        if (
            unit == self.BYTE_UNIT
            and contiguous_rows
            and contiguous_rows[0].offset == 0
            and len(contiguous_rows) == len(segment)
        ):
            total_size = sum(row.size for row in contiguous_rows)
            declared_size = self._find_total_size_statement(lines, table.start_line - 1)
            if declared_size is not None and declared_size != total_size:
                self.issues.append(
                    f"{table.file_path}:{table.start_line:d}: total-size statement says "
                    f"{declared_size:d} bytes but the table rows sum to {total_size:d} bytes"
                )

    def _contiguous_run(self, segment, unit):
        """Collect the leading verifiable rows of a segment."""
        contiguous_rows = []
        for line_number, offset_cell, size_cell in segment:
            try:
                if unit == self.BIT_UNIT:
                    offset = self._parse_offset(offset_cell, is_bits=True)
                    size = self._parse_bit_size(size_cell)
                    if size is None:
                        break
                else:
                    offset = self._parse_offset(offset_cell, is_bits=False)
                    size, is_variable = self._parse_size(size_cell)
                    if is_variable:
                        break
            except ValueError:
                break  # unparseable row -> stop this segment's run
            contiguous_rows.append(TableRowValue(line_number, offset, size))
        return contiguous_rows

    def _find_tables(self, file_path, lines):
        tables = []
        line_index = 0
        total_lines = len(lines)
        while line_index < total_lines:
            if lines[line_index].strip().startswith("| Offset | Size |"):
                table = OffsetTable(file_path, line_index + 1)
                line_index += 1
                if (
                    line_index < total_lines
                    and lines[line_index].strip().startswith("|")
                    and "---" in lines[line_index]
                ):
                    line_index += 1
                while line_index < total_lines and lines[line_index].strip().startswith(
                    "|"
                ):
                    parsed_row = self._parse_row(lines[line_index])
                    if parsed_row is not None:
                        cells, is_annotation = parsed_row
                        if is_annotation:
                            table.add_annotation_row(line_index + 1)
                        else:
                            table.add_row(line_index + 1, cells)
                    line_index += 1
                tables.append(table)
            else:
                line_index += 1
        return tables

    def _find_total_size_statement(self, lines, table_start_index):
        """Return the total size (bytes) named in the paragraph above the table,
        or None."""
        text = " ".join(reversed(self._paragraph_above(lines, table_start_index)))
        match = self.TOTAL_SIZE_PATTERN.search(text, re.I)
        if match is None:
            return None
        return int(match.group(1).replace(" ", "").replace("_", ""))

    def _is_bit_table(self, table):
        for row in table.rows:
            if row.is_annotation:
                continue
            if (
                self._parse_bit_size(row.size_cell) is not None
                or self.BIT_SIZE_PATTERN.match(row.size_cell) is not None
            ):
                return True
        return False

    def _paragraph_above(self, lines, table_start_index):
        """Lines of the non-blank paragraph directly above the table (back to
        the last heading or blank-line boundary)."""
        paragraph_lines = []
        index = table_start_index - 1
        while index >= 0:
            line = lines[index]
            if line.strip() == "":
                break
            paragraph_lines.append(line)
            if line.lstrip().startswith("#"):
                break
            index -= 1
        return paragraph_lines

    def _parse_bit_size(self, size_cell):
        match = self.BIT_SIZE_PATTERN.match(size_cell.strip())
        if match is None or match.group(1) == "...":
            return None
        return int(match.group(1))

    def _parse_offset(self, offset_cell, is_bits):
        """Parses the offset in bits (bit tables) or bytes (byte tables)."""
        stripped = offset_cell.strip().strip("*").strip()
        if not stripped:
            raise ValueError("empty offset")
        if is_bits:
            match = self.BIT_OFFSET_PATTERN.match(stripped)
            if match is None:
                raise ValueError(f"not a bit offset: {offset_cell!r}")
            bit_index = int(match.group(2) or 0)
            if bit_index >= 8:
                raise ValueError(f"bit offset {offset_cell!r}: bit index must be < 8")
            return int(match.group(1)) * 8 + bit_index
        # "76 (0x4c)" style is accepted too
        match = self.OFFSET_VALUE_PATTERN.match(stripped)
        if match is None:
            raise ValueError(f"not an offset: {offset_cell!r}")
        offset_token = match.group(1)
        base = 16 if offset_token.lower().startswith("0x") else 10
        return int(offset_token, base)

    def _parse_size(self, size_cell):
        """Parses (fixed_bytes, is_variable). fixed_bytes is in bytes."""
        stripped = size_cell.strip().strip("*").strip()
        if not stripped:
            return 0, True
        number_match = self.NUMBER_PATTERN.match(stripped)
        if number_match is not None:
            return int(number_match.group(1)), False
        product_match = self.PRODUCT_PATTERN.match(stripped)
        if product_match is not None:
            count = int(product_match.group(1))
            item_size = int(product_match.group(2))
            product = count * item_size
            if (
                product_match.group(3) is not None
                and int(product_match.group(3)) != product
            ):
                raise ValueError(
                    f"size {size_cell!r}: product is {product:d}, "
                    f"not {product_match.group(3)}"
                )
            return product, False
        if self.BIT_SIZE_PATTERN.match(stripped) is not None:
            return 0, False  # bit-sized; handled by the bit path
        return 0, True  # variable ("...", "N x entries", "(size)", ...)

    def _parse_row(self, line):
        """Return (cells, is_annotation) for a table row, or None if not a parsed row."""
        stripped = line.strip()
        if not stripped.startswith("|"):
            return None
        cells = [cell.strip() for cell in stripped.strip("|").split("|")]
        if any(cell.startswith("<td") for cell in cells):
            return cells, True  # annotation row (colspan / "Added in ...")
        if len(cells) < 2:
            return None
        return cells, False

    def _split_into_segments(self, table):
        """Split a table's rows into segments on annotation rows."""
        segments = []
        current_segment = []
        for row in table.rows:
            if row.is_annotation:
                if current_segment:
                    segments.append(current_segment)
                    current_segment = []
                continue
            current_segment.append((row.line_number, row.offset_cell, row.size_cell))
        if current_segment:
            segments.append(current_segment)
        return segments

    def validate_file(self, file_path):
        """Validates a Markdown file."""
        lines = file_path.read_text(errors="replace").split("\n")
        for table in self._find_tables(str(file_path), lines):
            self.validate_table(table, lines)

    def validate_table(self, table, lines):
        """Validates a table."""
        unit = self.BIT_UNIT if self._is_bit_table(table) else self.BYTE_UNIT
        segments = self._split_into_segments(table)
        runs = [self._contiguous_run(segment, unit) for segment in segments]
        for run in runs:
            self._check_contiguity(table, run, unit)
        if len(segments) == 1:
            self._check_total_size(table, lines, unit, runs[0], segments[0])


def main(command_line_arguments):
    file_paths = []
    for path_string in command_line_arguments or DEFAULT_PATHS:
        path = Path(path_string)
        if path.is_dir():
            file_paths.extend(sorted(path.glob("*.md")))
        elif path.is_file():
            file_paths.append(path)
        else:
            print(f"error: no such file or directory: {path}", file=sys.stderr)
            return 2

    if not file_paths:
        print("error: no Markdown files found", file=sys.stderr)
        return 2

    validator = OffsetTableValidator()
    for file_path in file_paths:
        try:
            validator.validate_file(file_path)
        except OSError as error:
            print(f"error: {error}", file=sys.stderr)
            return 2

    for issue in validator.issues:
        print(issue)
    if validator.issues:
        print(
            f"\n{len(validator.issues):d} issue(s) found "
            f"in {len(file_paths):d} file(s).",
            file=sys.stderr,
        )
        return 1

    print(f"OK: no offset/size table issues in {len(file_paths):d} file(s).")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
