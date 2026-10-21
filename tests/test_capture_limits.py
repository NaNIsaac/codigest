"""A capture file from a command that printed for hours is read from both ends only."""
from codigest.commands import term


def test_a_huge_capture_keeps_the_command_and_both_ends(tmp_path):
    file = tmp_path / "capture"
    body = b"x" * (5 * term.RAW_KEEP)
    file.write_bytes(b"npm run dev\0first line\n" + body + b"\nlast line\n")

    data = term._read_capture(file)

    command, _, output = data.partition(b"\0")
    assert command == b"npm run dev"
    assert output.startswith(b"first line\n")
    assert output.endswith(b"\nlast line\n")
    assert b"bytes elided by codigest" in output
    assert len(data) < 3 * term.RAW_KEEP


def test_a_normal_capture_is_read_whole(tmp_path):
    file = tmp_path / "capture"
    file.write_bytes(b"pytest\0all fine\n")
    assert term._read_capture(file) == b"pytest\0all fine\n"
