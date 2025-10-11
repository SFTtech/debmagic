#!/usr/bin/env python3

import os

from debmagic.v1 import Build, package, autodetect, buildflags, autotools

flags = buildflags.get_flags(maint_options="hardening=+all")
os.environ.update(flags)

if os.environ["DEB_HOST_ARCH_OS"] == "linux":
    configure_params = ["--enable-affinity", "--enable-delayacct"]
else:
    configure_params = ["--enable-hwloc"]

# hurd-i386 can open /proc (nothing there) and /proc/ which works
if os.environ["DEB_HOST_ARCH_OS"] == "hurd":
    configure_params += ["--with-proc=/proc/"]
else:
    configure_params += ["--enable-sensors"]


def configure(build: Build):
    autodetect.configure(
        build,
        ["--enable-openvz", "--enable-vserver", "--enable-unicode"] + configure_params,
    )


package(
    preset=autotools,
)
