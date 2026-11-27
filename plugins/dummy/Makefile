CC = gcc
RZ_CFLAGS := $(if $(wildcard ../../bin/include/librz/rz_asm.h),-I../../bin/include/librz -I../../bin/include/librz/sdb -I../../bin/include/librz/sflib -I../../bin/include,$(shell pkg-config --cflags rz_asm rz_util rz_lib 2>/dev/null))
CFLAGS = -Wall -Wextra -fPIC -O2 $(RZ_CFLAGS)
LDFLAGS = -shared

TARGET = librz_dummy_qir.so
SRC = rz_dummy_qir.c

all: $(TARGET)

$(TARGET): $(SRC)
	$(CC) $(CFLAGS) $(LDFLAGS) -o $@ $<

clean:
	rm -f $(TARGET)

.PHONY: all clean
