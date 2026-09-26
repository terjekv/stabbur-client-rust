#!/usr/bin/env python3
import copy
import hashlib
import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('release', Path(__file__).with_name('release.py'))
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)
IMAGE = 'ghcr.io/terjekv/stabbur-server@sha256:' + 'a' * 64


class ReleaseTests(unittest.TestCase):
    def manifest(self):
        return {'package': {'name': 'stabbur-cli', 'version': '0.0.1', 'metadata': {'stabbur': {
            'server-image': IMAGE, 'server-version': '0.0.1', 'client-version': '0.0.1'}}},
            'dependencies': {'stabbur_client': {'version': '=0.0.1', 'rev': 'b' * 40,
                             'git': 'https://github.com/terjekv/stabbur-client-rust'}}}

    def evidence(self):
        return {'image': IMAGE, 'client_compatibility': 'passed', 'cli_compatibility': 'passed',
                'sources': {name: 'b' * 40 for name in ('stabbur', 'stabbur-client-rust', 'stabbur-cli', 'stabbur-frontend')}}

    def test_unreleased_metadata_skips_publication(self):
        manifest = self.manifest()
        manifest['package']['metadata']['stabbur']['server-image'] = 'pending-server-release'
        self.assertIsNone(release.prepared(manifest, None))

    def test_cli_rejects_local_or_mutable_client_and_mismatched_image(self):
        valid = self.manifest()
        self.assertEqual(release.prepared(valid, self.evidence()), ('0.0.1', IMAGE))
        for key, value in [('path', '../client'), ('rev', 'main'), ('version', '^0.0.1')]:
            manifest = copy.deepcopy(valid)
            manifest['dependencies']['stabbur_client'][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                release.prepared(manifest, self.evidence())
        for evidence in [None, {**self.evidence(), 'image': IMAGE[:-1] + 'b'},
                         {**self.evidence(), 'cli_compatibility': 'failed'}]:
            with self.assertRaises(ValueError):
                release.prepared(valid, evidence)

    def test_complete_platform_packages_preserve_bytes_and_corruption_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            for platform, extension in [('linux-x86_64-musl', '.tar.gz'), ('linux-aarch64-musl', '.tar.gz'),
                                        ('macos-aarch64', '.tar.gz'), ('windows-x86_64', '.zip')]:
                path = directory / f'stabbur-{platform}-main{extension}'
                path.write_bytes(platform.encode())
                path.with_name(path.name + '.sha256').write_text(hashlib.sha256(path.read_bytes()).hexdigest() + '  ' + path.name + '\n')
            assets = release.cli_assets(directory, '0.0.1')
            self.assertEqual(len(assets), 8)
            for path in assets:
                self.assertTrue(path.is_file())
                self.assertIn('-v0.0.1', path.name)
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            path = directory / 'stabbur-linux-x86_64-musl-main.tar.gz'
            path.write_bytes(b'corrupt')
            path.with_name(path.name + '.sha256').write_text('0' * 64 + '  ' + path.name + '\n')
            with self.assertRaises(ValueError):
                release.cli_assets(directory, '0.0.1')
            self.assertTrue(path.exists())


if __name__ == '__main__':
    unittest.main()
