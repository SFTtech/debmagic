import shlex
import subprocess
import sys


def run_cmd(args: list[str], *popenargs, **kwargs):
    print(f"debmagic:\n {shlex.join(args)}")
    ret = subprocess.run(
        args, *popenargs, stdout=sys.stdout, stderr=sys.stderr, check=False, **kwargs
    )

    if ret.returncode != 0:
        sys.exit(1)  # TODO: proper error handling
