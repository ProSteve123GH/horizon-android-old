import re
import subprocess
import unittest
from pathlib import Path

SANDBOX = Path(__file__).resolve().parents[1] / 'packaging/android/mod_build_sandbox.sh'
# The only way a literal apostrophe may appear inside a single-quoted shell string.
ESCAPED = "'\"'\"'"


def embedded_scripts(source):
    for found in re.finditer(r"bash -euc '\n", source):
        start = found.end()
        pos = start
        while True:
            quote = source.index("'", pos)
            if source.startswith(ESCAPED, quote):
                pos = quote + len(ESCAPED)
                continue
            yield source[start:quote], source[quote + 1:]
            break


class ModBuildSandboxTests(unittest.TestCase):
    """The sandbox of the builds repository ran no build for as long as a raw apostrophe sat
    inside a comment of the script handed to `bash -euc '…'`: the quoting ended mid-comment,
    bash read half the script as a filename and every player's mod build failed with
    `keep: unbound variable` instead of compiling anything."""

    def test_each_embedded_script_ends_where_its_arguments_start(self):
        ends = [following.lstrip("\n") for _, following in embedded_scripts(SANDBOX.read_text())]
        self.assertEqual([e.split()[0] for e in ends], ["unsandboxed", "sandbox"])

    def test_syntax_check_passes(self):
        result = subprocess.run(["bash", "-n", str(SANDBOX)], capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr.decode())


if __name__ == "__main__":
    unittest.main()
