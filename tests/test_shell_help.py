from typer.testing import CliRunner

from codigest.commands import shell_help


def test_help_install_is_idempotent_and_uninstall_preserves_profile(tmp_path, monkeypatch):
    profile = tmp_path / "Microsoft.PowerShell_profile.ps1"
    profile.write_text("Set-Alias ll Get-ChildItem\n", encoding="utf-8")
    monkeypatch.setattr(shell_help.term, "_powershell_profiles", lambda: [profile])
    runner = CliRunner()

    assert runner.invoke(shell_help.app, ["install"]).exit_code == 0
    assert runner.invoke(shell_help.app, ["install"]).exit_code == 0
    content = profile.read_text(encoding="utf-8-sig")
    assert content.count(shell_help._BEGIN) == 1
    assert "Set-Alias ll Get-ChildItem" in content
    assert "help.ps1" in content
    assert profile.read_bytes().startswith(b"\xef\xbb\xbf")

    assert runner.invoke(shell_help.app, ["uninstall"]).exit_code == 0
    assert profile.read_text(encoding="utf-8-sig") == "Set-Alias ll Get-ChildItem\n"
    assert runner.invoke(shell_help.app, ["uninstall"]).exit_code == 0


def test_help_script_does_not_override_help_or_capture():
    source = shell_help._HELP_SCRIPT.read_text(encoding="utf-8")
    assert ".SYNOPSIS" in source
    assert "function global:codigest" in source
    assert "Get-Command codigest -CommandType Application" in source
    assert "function global:help" not in source
    assert "PSConsoleHostReadLine" not in source


def test_unavailable_powershell_fails_without_mutating_files(monkeypatch):
    monkeypatch.setattr(shell_help.term, "_powershell_profiles", list)
    result = CliRunner().invoke(shell_help.app, ["install"])
    assert result.exit_code == 1
    assert "PowerShell not found" in result.output
