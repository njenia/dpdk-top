APP = minimal_dpdk_app

PKGCONF ?= pkg-config

CFLAGS += -O2 -Wall $(shell $(PKGCONF) --cflags libdpdk)
LDFLAGS += $(shell $(PKGCONF) --libs libdpdk) -lpthread -lnuma -ldl

SRCS = main.c

$(APP): $(SRCS)
	$(CC) $(CFLAGS) -o $@ $^ $(LDFLAGS)

clean:
	rm -f $(APP)

.PHONY: clean
