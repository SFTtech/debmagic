# Copyright © 2026 Michael Loipführer <milo@sft.lol>
# Copyright © 2026 Jonas Jelten <jj@sft.lol>
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
package whose packaging instructions are written in Python with the
debmagic packaging API, in F<debian/rules.py>.

It is selected by setting the B<Build-Driver> field in F<debian/control>
to I<debmagic>:

    Source: mypackage
    ...
    Build-Driver: debmagic
    Build-Depends: debmagic-pkg, debmagic-dpkg-driver

The package needs to build-depend on I<debmagic-pkg> (the python
packaging API imported by F<debian/rules.py>) and on
I<debmagic-dpkg-driver> (this module), since dpkg-buildpackage loads
the driver from the build environment.

The driver executes F<debian/rules.py> with the dpkg build tasks
(clean, build, build-arch, build-indep, binary, binary-arch,
binary-indep and custom targets).

Root handling (B<Rules-Requires-Root>, gain-root-command, fakeroot) is
inherited from L<Dpkg::BuildDriver::DebianRules>, so debmagic packages
behave like any other package from dpkg-buildpackage's point of view.

B<Note>: This is a private module, its API can change at any time.

=cut

package Dpkg::BuildDriver::Debmagic 0.01;

use v5.36;

use parent qw(Dpkg::BuildDriver::DebianRules);

use Dpkg::Gettext;
use Dpkg::ErrorHandling;
use Dpkg::Path qw(find_command);

=head1 METHODS

=over 4

=item $bd = Dpkg::BuildDriver::Debmagic->new(%opts)

Create a new Dpkg::BuildDriver::Debmagic object.

When a package has B<Build-Driver: debmagic>, the build recipe will be loaded from F<debian/rules.py> by this driver.
The driver supports the same options as L<Dpkg::BuildDriver::DebianRules>.
The path to the rules file can be overridden with `dpkg-buildpackage -R B<debian_rules>`.

=cut

sub new {
    my ($this, %opts) = @_;
    my $class = ref($this) || $this;

    # dpkg-buildpackage provides its default rules path of 'debian/rules'.
    # an explicit -R override can be used to customize the load path.
    my $rules = $opts{debian_rules} // [ 'debian/rules.py' ];
    if (@{$rules} == 1 && $rules->[0] eq 'debian/rules') {
        $opts{debian_rules} = [ 'debian/rules.py' ];
    }

    return $class->SUPER::new(%opts);
}

=item $bd->pre_check()

Perform build driver specific checks, before anything else.

Apart from the F<debian/rules> checks inherited from
L<Dpkg::BuildDriver::DebianRules> (which apply to F<debian/rules.py>
alike), this verifies that a python3 interpreter is available.

=cut

sub pre_check {
    my $self = shift;

    $self->SUPER::pre_check();

    error(g_('this package requires the debmagic build driver, ' .
             'but python3 is not installed'))
        unless find_command('python3');

    return;
}

=back

=head1 CHANGES

=head2 Version 0.01

First version, marked private since the Dpkg::BuildDriver interface
itself is still experimental upstream.

=cut

1;

