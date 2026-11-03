"""Platform contract for the existing selected runtime build operation."""
import importlib.util
from pathlib import Path
from types import SimpleNamespace
from unittest import TestCase, main, mock

spec = importlib.util.spec_from_file_location('runtime_provider_under_test', Path(__file__).with_name('runtime_provider.py'))
provider = importlib.util.module_from_spec(spec)
spec.loader.exec_module(provider)

class SelectedRuntimeBuildTests(TestCase):
    def test_build_requests_cli_and_embed_from_the_same_selected_checkout(self):
        binary = provider.ROOT / 'target/release/mncs'
        with mock.patch.object(provider.subprocess, 'run', return_value=SimpleNamespace(returncode=0, stderr='')) as run, mock.patch.object(provider, 'inspect', return_value={'build_origin': {'status': 'matches-embedded-inputs'}}):
            result = provider.build(binary, 'cargo')
        command = ['cargo', 'build', '--offline', '--locked', '--release', '-p', 'mncs-cli', '-p', 'mncs-embed']
        self.assertEqual(run.call_args.args[0], command)
        self.assertEqual(run.call_args.kwargs['cwd'], provider.ROOT)
        self.assertEqual(result['build_command'], command)

if __name__ == '__main__':
    main()
