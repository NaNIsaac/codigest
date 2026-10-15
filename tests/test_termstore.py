from codigest.core import termstore
from codigest.core.termstore import Entry


def test_crlf_output_survives_normalization():
    assert termstore.normalize_output("a\r\nb\r\n") == "a\nb"


def test_progress_redraw_keeps_last_frame():
    assert termstore.normalize_output("10%\r50%\r100%\ndone") == "100%\ndone"


def test_render_escapes_quotes_in_command_attribute():
    out = termstore.render([Entry(command='python -c "print(1)"', output="1", exit_code=0)])
    assert 'command="python -c &quot;print(1)&quot;"' in out



def test_profile_block_strip_keeps_user_content():
    from codigest.commands import term
    block = f"{term._BEGIN}\nhook\n{term._END}\n"
    assert term._strip_block("Set-Alias g git\n\n" + block) == "Set-Alias g git\n"
    assert term._strip_block(block) == ""


if __name__ == "__main__":
    test_crlf_output_survives_normalization()
    test_progress_redraw_keeps_last_frame()
    test_render_escapes_quotes_in_command_attribute()
    test_profile_block_strip_keeps_user_content()
    print("ok")
