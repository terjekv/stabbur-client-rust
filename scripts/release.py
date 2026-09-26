#!/usr/bin/env python3
"""Validate pinned compatibility evidence and publish a versioned GitHub source/package release."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import tomllib


def prepared(manifest, evidence):
    package = manifest['package']
    metadata = package.get('metadata', {}).get('stabbur', {})
    image = metadata.get('server-image', '')
    if not image or image == 'pending-server-release':
        return None
    if not re.fullmatch(r'ghcr\.io/terjekv/stabbur-server@sha256:[a-f0-9]{64}', image):
        raise ValueError('The server image must use the exact published digest')
    if not evidence or evidence.get('image') != image or evidence.get('client_compatibility') != 'passed':
        raise ValueError('The configured image must match successful published compatibility evidence')
    if not all(re.fullmatch(r'[a-f0-9]{40}', evidence.get('sources', {}).get(name, ''))
               for name in ('stabbur', 'stabbur-client-rust', 'stabbur-cli', 'stabbur-frontend')):
        raise ValueError('Compatibility evidence must identify all four exact source revisions')
    if package['name'] == 'stabbur-cli':
        dependency = manifest['dependencies']['stabbur_client']
        if ('path' in dependency or dependency.get('git') != 'https://github.com/terjekv/stabbur-client-rust'
                or not re.fullmatch(r'[a-f0-9]{40}', dependency.get('rev', ''))
                or dependency.get('version') != '=' + metadata.get('client-version', '')
                or evidence.get('cli_compatibility') != 'passed'):
            raise ValueError('CLI releases require the exact released client source and successful image acceptance')
    return package['version'], image


def tag_commit(repository, tag):
    result = subprocess.run(['gh', 'api', f'repos/{repository}/git/ref/tags/{tag}'],
                            capture_output=True, text=True, timeout=30)
    if result.returncode:
        if 'HTTP 404' in result.stderr:
            return None
        raise RuntimeError('Unable to verify the existing release tag')
    target = json.loads(result.stdout)['object']
    if target['type'] == 'tag':
        target = json.loads(subprocess.check_output(['gh', 'api', f"repos/{repository}/git/tags/{target['sha']}"], timeout=30))['object']
    if target['type'] != 'commit':
        raise ValueError('Release tag must identify a commit')
    return target['sha']


def cli_assets(directory, version):
    assets = []
    for platform, extension in [('linux-x86_64-musl', '.tar.gz'), ('linux-aarch64-musl', '.tar.gz'),
                                ('macos-aarch64', '.tar.gz'), ('windows-x86_64', '.zip')]:
        source = directory / f'stabbur-{platform}-main{extension}'
        checksum = source.with_name(source.name + '.sha256').read_text().split()
        with source.open('rb') as contents:
            digest = hashlib.file_digest(contents, 'sha256').hexdigest()
        if checksum != [digest, source.name]:
            raise ValueError('A platform archive does not match its CI checksum')
        destination = directory / f'stabbur-{platform}-v{version}{extension}'
        source.rename(destination)
        proof = destination.with_name(destination.name + '.sha256')
        proof.write_text(f'{digest}  {destination.name}\n')
        assets.extend([destination, proof])
    return assets


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=('prepare', 'publish'))
    parser.add_argument('--assets', type=Path)
    args = parser.parse_args()
    manifest = tomllib.loads(Path('Cargo.toml').read_text())
    metadata = manifest['package'].get('metadata', {}).get('stabbur', {})
    evidence_path = Path('evidence') / ('server-' + metadata.get('server-version', 'pending') + '.json')
    evidence = json.loads(evidence_path.read_text()) if evidence_path.exists() else None
    result = prepared(manifest, evidence)
    if args.mode == 'prepare':
        if result:
            repository = {'stabbur_client': 'terjekv/stabbur-client-rust', 'stabbur-cli': 'terjekv/stabbur-cli'}[manifest['package']['name']]
            tag = 'v' + result[0]
            if tag_commit(repository, tag) is not None:
                subprocess.run(['gh', 'release', 'view', tag, '--repo', repository], check=True, timeout=30)
                print('This version is already released; choose a new version for another release')
                result = None
        with open(os.environ['GITHUB_OUTPUT'], 'a') as output:
            output.write('ready=' + str(result is not None).lower() + '\n')
            if result:
                output.write('image=' + result[1] + '\n')
                output.write('server_revision=' + evidence['sources']['stabbur'] + '\n')
                output.write('frontend_revision=' + evidence['sources']['stabbur-frontend'] + '\n')
                if manifest['package']['name'] == 'stabbur-cli':
                    output.write('client_revision=' + manifest['dependencies']['stabbur_client']['rev'] + '\n')
        return
    if result is None:
        raise ValueError('Release compatibility evidence is not ready')
    version, image = result
    repository = {'stabbur_client': 'terjekv/stabbur-client-rust', 'stabbur-cli': 'terjekv/stabbur-cli'}[manifest['package']['name']]
    event = json.loads(Path(os.environ['GITHUB_EVENT_PATH']).read_text())['workflow_run']
    sha = os.environ['GITHUB_SHA']
    if (os.environ['GITHUB_REPOSITORY'] != repository or event['head_sha'] != sha or
            event['head_branch'] != 'main' or event['event'] != 'push' or event['conclusion'] != 'success'
            or event['path'] != '.github/workflows/ci.yml'):
        raise ValueError('Release source must be the exact successful main CI revision')
    if manifest['package']['name'] == 'stabbur-cli':
        dependency = manifest['dependencies']['stabbur_client']
        if tag_commit('terjekv/stabbur-client-rust', 'v' + metadata['client-version']) != dependency['rev']:
            raise ValueError('The pinned public client revision must match its released tag')
    tag = 'v' + version
    existing = tag_commit(repository, tag)
    if existing is not None:
        if existing != sha:
            raise ValueError('The release version already identifies different source')
        subprocess.run(['gh', 'release', 'view', tag, '--repo', repository], check=True, timeout=30)
        return
    if args.assets is None:
        raise ValueError('Release assets are required')
    if manifest['package']['name'] == 'stabbur-cli':
        assets = cli_assets(args.assets, version)
    else:
        crate = args.assets / f'stabbur_client-{version}.crate'
        if not crate.is_file():
            raise ValueError('The verified client crate package is absent')
        assets = [crate]
    with tempfile.TemporaryDirectory(prefix='stabbur-release-') as temporary:
        notes = Path(temporary) / 'notes.md'
        notes.write_text(f"{manifest['package']['name']} {version}\n\nSource: `{sha}`\n\n"
                         f'Verified server: `{image}`\n\n'
                         f"Main CI: {event['html_url']}\n\n"
                         'This is a GitHub source/package release. The client is available by its immutable Git revision; '
                         'this workflow does not publish to crates.io.\n')
        acceptance = Path(temporary) / 'release-evidence.json'
        acceptance.write_text(json.dumps({**evidence, 'consumer_repository': repository,
            'consumer_revision': sha, 'consumer_ci_run': event['html_url'],
            'consumer_acceptance_run': os.environ['GITHUB_SERVER_URL'] + '/' + repository +
                '/actions/runs/' + os.environ['GITHUB_RUN_ID']}, indent=2) + '\n')
        subprocess.run(['gh', 'release', 'create', tag, '--repo', repository, '--target', sha,
                        '--title', f"{manifest['package']['name']} {version}", '--notes-file', str(notes),
                        *map(str, assets), str(evidence_path), str(acceptance), 'COMPATIBILITY.md'], check=True, timeout=180)


if __name__ == '__main__':
    main()
