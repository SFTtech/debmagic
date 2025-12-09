import subprocess
from unittest.mock import patch

from debmagic import _utils


@patch("subprocess.run", autospec=True)
def test_util_exec_str(mock_run):
    mock_run.return_value = subprocess.CompletedProcess(["some", "command"], returncode=0)
    _utils.run_cmd("some command")
    mock_run.assert_called_once_with(["some", "command"], check=True)


@patch("subprocess.run", autospec=True)
def test_util_exec_list(mock_run):
    mock_run.return_value = subprocess.CompletedProcess(["some", "command"], returncode=0)
    _utils.run_cmd(["some", "command"])
    mock_run.assert_called_once_with(["some", "command"], check=True)


@patch("subprocess.run", autospec=True)
def test_util_exec_str_dryrun(mock_run):
    _utils.run_cmd("some command", dry_run=True)
    mock_run.assert_not_called()


@patch("subprocess.run", autospec=True)
def test_util_exec_str_shlex(mock_run):
    mock_run.return_value = subprocess.CompletedProcess(["nothing"], returncode=0)
    _utils.run_cmd("some command 'some arg'")
    mock_run.assert_called_once_with(["some", "command", "some arg"], check=True)


@patch("subprocess.run", autospec=True)
def test_util_exec_str_check(mock_run):
    mock_run.return_value = subprocess.CompletedProcess(["something"], returncode=0)
    _utils.run_cmd("something", check=True)
    mock_run.assert_called_once_with(["something"], check=True)


@patch("subprocess.run", autospec=True)
def test_util_exec_str_nocheck(mock_run):
    mock_run.return_value = subprocess.CompletedProcess(["something"], returncode=0)
    _utils.run_cmd("something", check=False)
    mock_run.assert_called_once_with(["something"], check=False)
