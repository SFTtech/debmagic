ROOT_DIR:=$(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))

product = test.changes

references = $(ROOT_DIR)/referenced-files

.DEFAULT_GOAL := $(product)

$(product): $(ROOT_DIR)/$(product)
	if [ -d $(references) ] ; then \
		cp -rp $(references)/. . ; \
	fi
	cp $(ROOT_DIR)/$(product) $(product)

.PHONY: clean
clean:
	rm -f $(product)
