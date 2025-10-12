import io
import shlex
import subprocess
import sys


class Namespace:
    """
    Store dict as object.
    """

    def __init__(self, **kwargs):
        for name in kwargs:
            setattr(self, name, kwargs[name])

    def __eq__(self, other):
        if not isinstance(other, Namespace):
            return NotImplemented
        return vars(self) == vars(other)

    def __contains__(self, key):
        return key in self.__dict__

    def __hash__(self):
        return hash(self.__dict__)

    def __getitem__(self, key):
        return self.__dict__[key]

    def get(self, key):
        return self.__dict__.get(key)


def run_cmd(args: list[str], *popenargs, **kwargs):
    print(f"debmagic: {shlex.join(args)}")
    ret = subprocess.run(
        args, check=False, *popenargs, **kwargs
    )

    if ret.returncode != 0:
        sys.exit(1)  # TODO: proper error handling


def disable_output_buffer():
    # always flush output
    sys.stdout = io.TextIOWrapper(open(sys.stdout.fileno(), 'wb', 0), write_through=True)
    sys.stderr = io.TextIOWrapper(open(sys.stderr.fileno(), 'wb', 0), write_through=True)
