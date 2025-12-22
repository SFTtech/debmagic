import io
import os
import re
import shlex
import shutil
import signal
import subprocess
import sys
from pathlib import Path
from typing import Callable, Sequence, TypeVar


class Namespace:
    """
    Store dict as object.
    """

    def __init__(self, **kwargs):
        for name, value in kwargs.items():
            setattr(self, name, value)

    def __eq__(self, other):
        if not isinstance(other, Namespace):
            return NotImplemented
        return vars(self) == vars(other)

    def __contains__(self, key):
        return key in self.__dict__

    def __hash__(self):
        return hash(self.__dict__)

    def __getattr__(self, key):
        return self.__dict__[key]

    def __getitem__(self, key):
        return self.__dict__[key]

    def get(self, key):
        return self.__dict__.get(key)


def run_cmd(
    cmd: Sequence[str | Path] | str,
    check: bool = True,
    dry_run: bool = False,
    **kwargs,
) -> subprocess.CompletedProcess:
    cmd_args: Sequence[str | Path] | str = cmd
    cmd_pretty: str

    if kwargs.get("shell"):
        if not isinstance(cmd, str):
            cmd_args = shlex.join([str(x) for x in cmd])
    else:
        if isinstance(cmd, str):
            cmd_args = shlex.split(cmd)

    if isinstance(cmd, str):
        cmd_pretty = cmd
    else:
        cmd_pretty = shlex.join([str(x) for x in cmd_args])

    print(f"debmagic: {cmd_pretty}")

    if dry_run:
        return subprocess.CompletedProcess(cmd_args, 0)

    ret = subprocess.run(cmd_args, check=check, **kwargs)

    return ret


def run_cmd_in_foreground(args: Sequence[str | Path], **kwargs):
    """
    the "correct" way of spawning a new subprocess:
    signals like C-c must only go
    to the child process, and not to this python.

    the args are the same as subprocess.Popen

    returns Popen().wait() value

    Some side-info about "how ctrl-c works":
    https://unix.stackexchange.com/a/149756/1321

    fun fact: this function took a whole night
              to be figured out.
    """
    cmd_pretty = shlex.join([str(x) for x in args])
    print(f"debmagic[fg]: {cmd_pretty}")

    # import here to only use the dependency if really necessary (not available on Windows)
    import termios

    old_pgrp = os.tcgetpgrp(sys.stdin.fileno())
    old_attr = termios.tcgetattr(sys.stdin.fileno())

    user_preexec_fn: Callable | None = kwargs.pop("preexec_fn", None)

    def new_pgid():
        if user_preexec_fn:
            user_preexec_fn()

        # set a new process group id
        os.setpgid(os.getpid(), os.getpid())

        # generally, the child process should stop itself
        # before exec so the parent can set its new pgid.
        # (setting pgid has to be done before the child execs).
        # however, Python 'guarantee' that `preexec_fn`
        # is run before `Popen` returns.
        # this is because `Popen` waits for the closure of
        # the error relay pipe '`errpipe_write`',
        # which happens at child's exec.
        # this is also the reason the child can't stop itself
        # in Python's `Popen`, since the `Popen` call would never
        # terminate then.
        # `os.kill(os.getpid(), signal.SIGSTOP)`

    try:
        # fork the child
        child = subprocess.Popen(args, preexec_fn=new_pgid, **kwargs)  # noqa: PLW1509

        # we can't set the process group id from the parent since the child
        # will already have exec'd. and we can't SIGSTOP it before exec,
        # see above.
        # `os.setpgid(child.pid, child.pid)`

        # set the child's process group as new foreground
        os.tcsetpgrp(sys.stdin.fileno(), child.pid)
        # revive the child,
        # because it may have been stopped due to SIGTTOU or
        # SIGTTIN when it tried using stdout/stdin
        # after setpgid was called, and before we made it
        # forward process by tcsetpgrp.
        os.kill(child.pid, signal.SIGCONT)

        # wait for the child to terminate
        ret = child.wait()

    finally:
        # we have to mask SIGTTOU because tcsetpgrp
        # raises SIGTTOU to all current background
        # process group members (i.e. us) when switching tty's pgrp
        # it we didn't do that, we'd get SIGSTOP'd
        hdlr = signal.signal(signal.SIGTTOU, signal.SIG_IGN)
        # make us tty's foreground again
        os.tcsetpgrp(sys.stdin.fileno(), old_pgrp)
        # now restore the handler
        signal.signal(signal.SIGTTOU, hdlr)
        # restore terminal attributes
        termios.tcsetattr(sys.stdin.fileno(), termios.TCSADRAIN, old_attr)

    return ret


def disable_output_buffer():
    # always flush output
    sys.stdout = io.TextIOWrapper(open(sys.stdout.fileno(), "wb", 0), write_through=True)
    sys.stderr = io.TextIOWrapper(open(sys.stderr.fileno(), "wb", 0), write_through=True)


def prefix_idx(prefix: str, seq: list[str]) -> int:
    """
    >>> prefix_idx("a", ["c", "a", "d"])
    1
    >>> prefix_idx("a", ["c", "d", "e", "a"])
    3
    """
    for idx, elem in enumerate(seq):
        if re.match(rf"{prefix}\b", elem):
            return idx

    raise ValueError(f"prefix {prefix!r} not found in sequence")


T = TypeVar("T")


def list_strip_head(data: list[T], head: list[T]) -> list[T]:
    """
    >>> list_strip_head([1,2,3], [])
    [1, 2, 3]
    >>> list_strip_head([1,2,3], [1,2])
    [3]
    >>> list_strip_head([1,2,3], [1,2,3])
    []
    >>> list_strip_head([1,2,3,4,5], [1,2])
    [3, 4, 5]
    """
    idx = 0
    for elem_a, elem_b in zip(data, head, strict=False):
        if elem_a == elem_b:
            idx += 1
        else:
            break

    return data[idx:]


def copy_file_if_exists(source: Path, glob: str, dest: Path):
    for file in source.glob(glob):
        if file.is_dir():
            shutil.copytree(file, dest)
        elif file.is_file():
            shutil.copy(file, dest)
        else:
            raise NotImplementedError("Don't support anything besides files and directories")


if __name__ == "__main__":
    import doctest

    doctest.testmod()
