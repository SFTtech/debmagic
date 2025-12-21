from pathlib import Path

from debmagic.cli._config import get_config_argparser, load_config


def test_config_load():
    config_file = Path(__file__).parent / "assets" / "config1.toml"
    parser = get_config_argparser()
    args = parser.parse_args(["--temp-build-dir", "/tmp/bla"])
    config = load_config(args, [config_file])

    assert config.dry_run
    assert config.temp_build_dir == Path("/tmp/bla")
