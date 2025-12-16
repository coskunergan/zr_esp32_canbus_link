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

#define GET_SWAP32(val) \
    ((((uint32_t)(val) & 0x000000FF) << 24) | \
      (((uint32_t)(val) & 0x0000FF00) << 8)  | \
      (((uint32_t)(val) & 0x00FF0000) >> 8)  | \
      (((uint32_t)(val) & 0xFF000000) >> 24))

#define STM32_FLASH_BASE    	0x08000000
#define WRITE_CHUNK_SIZE    	256
#define MAX_STM32_FLASH_SIZE    1500000
#define MAX_EXT_FLASH_SIZE  	4000000
#define TLV_IMG_HEADER_SIZE 	0x200

#define IMG_SIZE_OFFSET     12
#define HEADER_READ_SIZE    64

int stm32_flashing_start(uint32_t fw_offset, uint32_t fw_size, uint32_t target_addr, bool boot_start);