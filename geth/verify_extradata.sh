#!/bin/bash

EXTRADATA=$1

# Remove "0x" prefix if present
EXTRADATA=${EXTRADATA#0x}

# Expected lengths in hex characters
ZERO_PREFIX_LEN=64  # 32 bytes of zeros
SIGNER_LEN=40       # 20-byte signer address
ZERO_SUFFIX_LEN=130 # 65 bytes of zeros
TOTAL_LEN=$((ZERO_PREFIX_LEN + SIGNER_LEN + ZERO_SUFFIX_LEN))

# Check total length
if [ ${#EXTRADATA} -ne $TOTAL_LEN ]; then
    echo "❌ Invalid extradata length: Expected $TOTAL_LEN, but got ${#EXTRADATA}"
    exit 1
fi

# Check leading zeros
LEADING_ZEROS=${EXTRADATA:0:$ZERO_PREFIX_LEN}
if [[ ! "$LEADING_ZEROS" =~ ^0+$ ]]; then
    echo "❌ Leading zeros are incorrect"
    exit 1
fi

# Extract signer address
SIGNER=${EXTRADATA:$ZERO_PREFIX_LEN:$SIGNER_LEN}
echo "✅ Signer Address: 0x$SIGNER"

# Check trailing zeros
TRAILING_ZEROS=${EXTRADATA:$((ZERO_PREFIX_LEN + SIGNER_LEN)):$ZERO_SUFFIX_LEN}
if [[ ! "$TRAILING_ZEROS" =~ ^0+$ ]]; then
    echo "❌ Trailing zeros are incorrect"
    exit 1
fi

echo "✅ Extradata format is correct!"
exit 0