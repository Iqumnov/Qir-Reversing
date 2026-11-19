CC = gcc
CFLAGS = -Wall -Wextra -fPIC -O2 \
         -I../bin/include/librz \
         -I../bin/include/librz/sdb \
         -I../bin/include/librz/sflib \
         -I../bin/include
LDFLAGS = -shared

TARGET = librz_dummy_qir.so
SRC = rz_dummy_qir.c

all: $(TARGET)

$(TARGET): $(SRC)
	$(CC) $(CFLAGS) $(LDFLAGS) -o $@ $<

clean:
	rm -f $(TARGET)

.PHONY: all clean
