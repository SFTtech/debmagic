#!/usr/bin/python3

from asyncore import socket_map

# This should not trigger the uses-deprecated-python-stdlib tag
from ..server.asyncserver import asyncore, RequestHandler, loop, AsyncServer, AsyncServerException
from supervisor.medusa import asyncore_25 as asyncore
from notasyncore import socket_map
