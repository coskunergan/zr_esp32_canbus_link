// Copyright (c) 2025
// SPDX-License-Identifier: Apache-2.0
// Coskun ERGAN <coskunergan@gmail.com>

#include <zephyr/kernel.h>
#include <zephyr/canbus/isotp.h>
#include <zephyr/sys/util.h>
#include <zephyr/logging/log.h>

#include "mongoose_glue.h"

void mg_poll()
{
    mongoose_poll();
}

int mg_init()
{
    mongoose_init();
    return 0;
}
