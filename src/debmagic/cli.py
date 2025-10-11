import argparse

from debmagic._utils import run_cmd


def parse_args():
    parser = argparse.ArgumentParser(description="Debmagic")
    return parser.parse_args()


def main():
    args = parse_args()
    # just call debuild for now
    run_cmd(["debuild", "-nc", "-uc", "-b"])
