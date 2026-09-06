ROOT_DIR:=$(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))

name = fields-general-missing

package = $(name).deb

datadir = $(ROOT_DIR)/root
controldir = $(ROOT_DIR)/DEBIAN

builddir = build
debiandir = $(builddir)/DEBIAN
rootdir = $(builddir)/root
docdir = $(rootdir)/usr/share/doc/$(name)

mandatorydir = $(ROOT_DIR)/doc
changelogfile = $(mandatorydir)/changelog
copyrightfile = $(mandatorydir)/copyright

version = 1.0-1

ifeq "$(findstring -, $(version))" ""
changelogtarget = $(docdir)/changelog
else
changelogtarget = $(docdir)/changelog.Debian
endif

pre_build = $(ROOT_DIR)/pre-build
pre_data = $(ROOT_DIR)/pre-data
tar_data = $(ROOT_DIR)/tar-data
pre_control = $(ROOT_DIR)/pre-control
tar_control = $(ROOT_DIR)/tar-control
post_build = $(ROOT_DIR)/post-build

deb_member_source_path = $(ROOT_DIR)/members
deb_member_build_path = members

debmembers = $(shell tr '\n' ' ' < $(ROOT_DIR)/deb-members)
controlmembers = $(shell tr '\n' ' ' < $(ROOT_DIR)/control-members)

$(package): fix-perm
	-mkdir -p $(deb_member_build_path)
	if [ -d $(deb_member_source_path) \
		-a $(deb_member_source_path) != $(deb_member_build_path) ] ; then \
		cp -rp $(deb_member_source_path)/. $(deb_member_build_path) ; \
	fi

	-mkdir -p $(rootdir)
	if [ -d $(datadir) ] ; then \
		cp -rp $(datadir)/. $(rootdir) ; \
	fi

	if [ -x $(pre_build) ] ; then \
		$(pre_build) "$(rootdir)" "$(name)" ; \
	fi

	-mkdir -p $(docdir)
	-if [ -f $(changelogfile) ] ; then \
		install --mode 0644 $(changelogfile) $(changelogtarget) ; \
		gzip -9nf $(changelogtarget) ; \
	fi
	if [ -f $(copyrightfile) ] ; then \
		install --mode 0644 $(copyrightfile) $(docdir) ; \
	fi

	# pass package name to pre-data script
	if [ -x $(pre_data) ] ; then \
		$(pre_data) "$(rootdir)" "$(name)" ; \
	fi

	if [ -x $(tar_data) ] ; then \
		$(tar_data) "$(rootdir)" ; \
	fi

	-mkdir $(debiandir)
	if [ -d $(controldir) ] ; then \
		cp -rp $(controldir)/. $(debiandir) ; \
	fi

	# prevents 'control-file-has-bad-permissions' on CI runners with strange umasks
	find $(debiandir) -type d -exec chmod 0755 \{\} +
	find $(debiandir) -type f -exec chmod 0644 \{\} +

	(cd $(rootdir); find . -type f -a -! -ipath etc -exec md5sum -- \{\} +) > $(debiandir)/md5sums

	# pass package name to pre-control script
	if [ -x $(pre_control) ] ; then \
		$(pre_control) "$(debiandir)" "$(name)" ; \
	fi
	if [ -x $(tar_control) ] ; then \
		$(tar_control) "$(debiandir)" "$(controlmembers)" ; \
	fi

	ar rc $(package) $(debmembers)

	# pass package name to post-build script
	if [ -x $(post_build) ] ; then \
		$(post_build) "$(name)" ; \
	fi


.PHONY: clean
clean:
	rm -f $(package)
	rm -rf $(builddir)

# If root/ exists, it is because the test ships with it.  Since the
# test may have been checked out (or unpacked) with a "whack umask"
# (anything but 0022), we reset the permissions to a reasonable
# default.
#
# The contents of the deb usually is not what is tested by this suite
# (t/tests is preferred for this), so the below merely handles the
# AVERAGE CASE.  Tests that need special permissions (anything but
# 0644 for files and 0755 for dirs) require manually setting the
# permissions.
.PHONY: fix-perm
fix-perm:
	if [ -d $(datadir) ] ; then \
		find $(datadir) -type d | xargs -r chmod 0755 ; \
		find $(datadir) -type f | xargs -r chmod 0644 ; \
	fi
