import argparse


def parse_args():
    parser = argparse.ArgumentParser(description="Debmagic")
    return parser.parse_args()


def main():
    args = parse_args()
    print("Hello from debmagic")
