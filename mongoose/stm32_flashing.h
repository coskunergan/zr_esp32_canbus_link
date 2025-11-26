// Copyright (c) 2025
// SPDX-License-Identifier: Apache-2.0
// Coskun ERGAN <coskunergan@gmail.com>
#pragma once

#define GET_LE16(p) \
    (((uint32_t)((p)[0])) | \
     ((uint32_t)((p)[1]) << 8))

#define GET_LE32(p) \
    (((uint32_t)((p)[0])) | \
     ((uint32_t)((p)[1]) << 8) | \
     ((uint32_t)((p)[2]) << 16) | \
     ((uint32_t)((p)[3]) << 24))


#define STM32_FLASH_BASE    0x08000000
#define WRITE_CHUNK_SIZE    256
#define MAX_FLASH_SIZE      1500000
#define TLV_IMG_HEADER_SIZE 0x200

#define IMG_SIZE_OFFSET     12
#define HEADER_READ_SIZE    64

int check_magic(void);
int stm32_flashing_start(void);