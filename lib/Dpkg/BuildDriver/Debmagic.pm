# Copyright © 2025 Michael Loipführer <milo@sft.lol>
#
# This program is free software; you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation; either version 2 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with this program.  If not, see <https://www.gnu.org/licenses/>.

=encoding utf8

=head1 NAME

Dpkg::BuildDriver::Debmagic - build a Debian package using debmagic

=head1 DESCRIPTION

This class is used by dpkg-buildpackage to drive the build of a Debian
package, using debmagic.

B<Note>: This is a private module, its API can change at any time.

=cut

package Dpkg::BuildDriver::Debmagic 0.01;
use strict;
use warnings FATAL => 'all';
use Dpkg::ErrorHandling;

sub _run_cmd {
    my @cmd = @_;
    printcmd(@cmd);
    system @cmd and subprocerr("@cmd");
}

sub new {
    my ($this, %opts) = @_;
    my $class = ref($this) || $this;
    my $self = bless({
        'ctrl' => $opts{ctrl},
        'debmagic_cmd' => 'debmagic',
    }, $class);
    return $self;
}


sub need_build_task {
    return 0;
}

sub run_task {
    my ($self, $task) = @_;
    _run_cmd($self->{'debmagic_cmd'}, 'internal-command', 'dpkg-build-driver-run-task', $task);
    return;
}

1;

