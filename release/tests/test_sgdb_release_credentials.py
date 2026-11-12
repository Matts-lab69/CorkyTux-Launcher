"""The release executable must ignore the retired build-time credential input."""

import os
from pathlib import Path
import unittest


BUILD_CANARY = b"SGDB_BUILD_CANARY_NOT_A_CREDENTIAL_20261112"


@unittest.skipUnless(os.environ.get("CORKYTUX_RELEASE_BINARY"), "release artifact check runs after build")
class ReleaseCredentialTests(unittest.TestCase):
    def test_retired_build_input_is_not_embedded(self):
        binary = Path(os.environ["CORKYTUX_RELEASE_BINARY"])
        self.assertTrue(binary.is_file(), "release executable is missing")
        self.assertFalse(BUILD_CANARY in binary.read_bytes(), "retired build input was embedded")


if __name__ == "__main__":
    unittest.main()
