"""`cdg cap` options in front of the command, and PowerShell `{ }` blocks."""

import base64

import pytest

from codigest.commands.cap import script_block, split_options
from codigest.core.formats import Format


def test_options_before_the_command_are_read_and_the_rest_is_left_alone():
    delivery, command = split_options(["-c", "--format=markdown", "pytest", "-s", "-c"])
    assert delivery.requested and delivery.copy and not delivery.stdout
    assert delivery.format is Format.MARKDOWN
    assert command == ["pytest", "-s", "-c"]


def test_without_options_nothing_is_handed_over():
    delivery, command = split_options(["pytest", "-q"])
    assert not delivery.requested
    assert command == ["pytest", "-q"]


def test_a_double_dash_ends_the_options():
    delivery, command = split_options(["-s", "--", "-c", "x"])
    assert delivery.stdout
    assert command == ["-c", "x"]


def test_an_unknown_format_is_an_error():
    with pytest.raises(ValueError, match="unknown format"):
        split_options(["--format", "yaml", "ls"])


def test_a_block_from_powershell_is_decoded():
    # What PowerShell passes to a program for `{ ls }`.
    encoded = base64.b64encode(" ls ".encode("utf-16-le")).decode()
    assert encoded == "IABsAHMAIAA="
    assert (
        script_block(["-encodedCommand", encoded, "-inputFormat", "xml", "-outputFormat", "text"])
        == " ls "
    )
    assert script_block(["pytest", "-q"]) is None
