use super::Fixture;

impl Fixture {
    /// Python, MATLAB and JS files that import each other, for `--resolve` and `digest`.
    pub fn code() -> Self {
        let fx = Self::new();
        fx.write(
            "main.py",
            r#""""Entry point."""
from pkg import util
import helper
import os


class App(Base):
    """The app.

    More.
    """

    def run(self, a: int = 1, *rest, key=None) -> str:
        """Run it."""
        return helper.go(a)

    @property
    def name(self):
        return 'x'


async def serve(host='h', port=80):
    pass
"#,
        );
        fx.write("helper.py", "def go(x):\n    return x\n\n\nVALUE = 3\n");
        fx.write("pkg/__init__.py", "");
        fx.write(
            "pkg/util.py",
            "from .deep import d\n\n\ndef u():\n    return d\n",
        );
        fx.write("pkg/deep.py", "d = 1\n");
        fx.write("unused.py", "def never():\n    pass\n");
        fx.write("stubs.pyi", "def f(x: int) -> str: ...\n");
        fx.write(
            "calc.m",
            "function y = calc(x)\n% CALC doubles x\n  y = 2*x;\nend\n",
        );
        fx.write("crlf.py", "def a():\r\n    \"\"\"doc\"\"\"\r\n    pass\r\n");
        fx.write("broken.py", "def (:\n");
        fx.write("empty.py", "");
        fx.write(
            "app.js",
            "import { x } from './lib/x.js';\nconst y = require('./lib/y');\n",
        );
        fx.write("lib/x.ts", "export const x = 1;\n");
        fx.write("lib/y.js", "module.exports = 1;\n");
        fx.write("lib/z.js", "module.exports = 2;\n");
        fx.bytes("latin.py", b"def f():\n    \"\"\"caf\xe9\"\"\"\n");
        fx
    }
}
