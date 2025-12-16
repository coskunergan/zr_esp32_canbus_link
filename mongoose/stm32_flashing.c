// Copyright (c) 2025
// SPDX-License-Identifier: Apache-2.0
// Coskun ERGAN <coskunergan@gmail.com>

#include <zephyr/kernel.h>
#include <zephyr/device.h>
#include <zephyr/drivers/gpio.h>
#include <zephyr/drivers/uart.h>
#include <zephyr/drivers/flash.h>
#include <zephyr/storage/flash_map.h>
#include <zephyr/logging/log.h>
#include <zephyr/drivers/uart.h>
#include "stm32_flashing.h"

LOG_MODULE_REGISTER(main, LOG_LEVEL_INF);

#define UART_DEV        DEVICE_DT_GET(DT_ALIAS(uart1))
#define BOOT0_NODE      DT_NODELABEL(boot0_pin)
#define NRST_NODE       DT_NODELABEL(nrst_pin)

static const struct gpio_dt_spec boot0 = GPIO_DT_SPEC_GET(BOOT0_NODE, gpios);
static const struct gpio_dt_spec nrst  = GPIO_DT_SPEC_GET(NRST_NODE,  gpios);
static const struct device *uart_dev = DEVICE_DT_GET(DT_NODELABEL(uart1));

static uint8_t calculate_checksum(const uint8_t *data, uint16_t len)
{
    uint8_t checksum = 0;
    for(uint16_t i = 0; i < len; i++)
    {
        checksum ^= data[i];
    }
    return checksum;
}

static int wait_ack(int timeout_ms)
{
    uint8_t ack = 0xff;
    int64_t start = k_uptime_get();

    while(k_uptime_get() - start < timeout_ms)
    {
        if(uart_poll_in(UART_DEV, &ack) == 0)
        {
            if(ack == 0x79)
            {
                return 0;
            }
            else if(ack == 0x1F)
            {
                LOG_ERR("NACK alındı!");
                return -EIO;
            }
            LOG_ERR("ACK : %x", ack);
        }
        k_usleep(500);
    }
    LOG_ERR("ACK timeout!");

    return -ETIMEDOUT;
}

static int send_address(uint32_t address)
{
    uint8_t addr[5];
    addr[0] = (address >> 24) & 0xFF;
    addr[1] = (address >> 16) & 0xFF;
    addr[2] = (address >> 8) & 0xFF;
    addr[3] = address & 0xFF;
    addr[4] = calculate_checksum(addr, 4);

    for(int i = 0; i < 5; i++)
    {
        uart_poll_out(UART_DEV, addr[i]);
    }

    return wait_ack(500);
}

static int send_data(const uint8_t *data, uint32_t len)
{
    uint8_t n = len - 1;
    uart_poll_out(UART_DEV, n);

    uint8_t checksum = n;
    for(uint32_t i = 0; i < len; i++)
    {
        uart_poll_out(UART_DEV, data[i]);
        checksum ^= data[i];
    }
    uart_poll_out(UART_DEV, checksum);

    return wait_ack(500);
}

static int stm32_read_memory(uint32_t address, uint8_t *data, uint32_t len)
{
    if(len > 256 || len == 0)
    {
        return -EINVAL;
    }
    uart_poll_out(UART_DEV, 0x11);
    uart_poll_out(UART_DEV, 0xEE);

    if(wait_ack(500) < 0)
    {
        LOG_ERR("Read command ACK failed");
        return -EIO;
    }

    if(send_address(address) < 0)
    {
        LOG_ERR("Address send failed");
        return -EIO;
    }

    uint8_t len_byte = len - 1;
    uart_poll_out(UART_DEV, len_byte);
    uart_poll_out(UART_DEV, ~len_byte);

    if(wait_ack(500) < 0)
    {
        LOG_ERR("Read length ACK failed");
        return -EIO;
    }

    for(uint32_t i = 0; i < len; i++)
    {
        int64_t start = k_uptime_get();
        while(k_uptime_get() - start < 500)
        {
            if(uart_poll_in(UART_DEV, &data[i]) == 0)
            {
                break;
            }
            k_usleep(100);
        }
    }
    return 0;
}

static int stm32_write_memory(uint32_t address, const uint8_t *data, uint32_t len)
{
    if(len > 256 || len == 0)
    {
        return -EINVAL;
    }
    uart_poll_out(UART_DEV, 0x31);
    uart_poll_out(UART_DEV, ~0x31);

    if(wait_ack(500) < 0)
    {
        LOG_ERR("Write command ACK failed");
        return -EIO;
    }

    if(send_address(address) < 0)
    {
        LOG_ERR("Address send failed");
        return -EIO;
    }

    if(send_data(data, len) < 0)
    {
        LOG_ERR("Data send failed");
        return -EIO;
    }

    return 0;
}


static int stm32_verify_firmware(uint32_t fw_offset, uint32_t fw_size, uint32_t target_addr)
{
    const struct flash_area *fa;

    LOG_INF("Firmware doğrulaması başlıyor...");

    int ret = flash_area_open(FIXED_PARTITION_ID(slot1_partition), &fa);
    if(ret < 0)
    {
        LOG_ERR("Partition açma başarısız: %d", ret);
        return ret;
    }

    uint8_t partition_buffer[WRITE_CHUNK_SIZE];
    uint8_t stm32_buffer[WRITE_CHUNK_SIZE];
    uint32_t offset = 0;
    uint32_t last_log_offset = 0;
    uint32_t mismatch_count = 0;

    while(offset < fw_size)
    {
        uint32_t chunk_size = MIN(WRITE_CHUNK_SIZE, fw_size - offset);

        ret = flash_area_read(fa, fw_offset + offset, partition_buffer, chunk_size);
        if(ret < 0)
        {
            LOG_ERR("Partition okuma başarısız (offset: %u): %d", offset, ret);
            flash_area_close(fa);
            return ret;
        }

        int ret = stm32_read_memory(target_addr, stm32_buffer, chunk_size);
        if(ret < 0)
        {
            LOG_ERR("STM32 okuma başarısız (adres: 0x%08X): %d", target_addr, ret);
            flash_area_close(fa);
            return ret;
        }

        for(uint32_t i = 0; i < chunk_size; i++)
        {
            if(partition_buffer[i] != stm32_buffer[i])
            {
                mismatch_count++;
                LOG_WRN("Veri uyuşmazlığı - Adres: 0x%08X, Beklenen: 0x%02X, Okunan: 0x%02X",
                        target_addr + i, partition_buffer[i], stm32_buffer[i]);
            }
        }

        offset += chunk_size;
        target_addr += chunk_size;

        if(offset - last_log_offset >= 1024 * 5)
        {
            uint32_t percent = (offset * 100) / fw_size;
            LOG_INF("Doğrulama: %u%% (%u/%u bytes)", percent, offset, fw_size);
            last_log_offset = offset;
        }
    }
    flash_area_close(fa);

    if(mismatch_count == 0)
    {
        LOG_INF("✓ Doğrulama başarılı! Hiçbir veri uyuşmazlığı yok.");
        return 0;
    }
    else
    {
        LOG_ERR("✗ Doğrulama başarısız! %u byte uyuşmazlık bulundu.", mismatch_count);
        return -1;
    }
}

static int stm32_write_firmware(uint32_t fw_offset, uint32_t fw_size, uint32_t target_addr)
{
    const struct flash_area *fa;

    LOG_INF("Firmware yazma işlemi başlıyor...");

    int ret = flash_area_open(FIXED_PARTITION_ID(slot1_partition), &fa);
    if(ret < 0)
    {
        LOG_ERR("Partition açma başarısız: %d", ret);
        return ret;
    }

    LOG_INF("Partition boyutu: %u bytes (%.1f KB)", fw_size, fw_size / 1024.0);

    uart_poll_out(UART_DEV, 0x44);
    uart_poll_out(UART_DEV, 0xBB);

    if(wait_ack(500) < 0)
    {
        LOG_ERR("Prepare command ACK failed");
        flash_area_close(fa);
        return -EIO;
    }

    uart_poll_out(UART_DEV, 0x00);
    uart_poll_out(UART_DEV, 0x00);
    uart_poll_out(UART_DEV, 0x00);
    uart_poll_out(UART_DEV, 0x00);
    uart_poll_out(UART_DEV, 0x00);

    if(wait_ack(2500) < 0)
    {
        LOG_ERR("Prapera write command ACK failed");
        flash_area_close(fa);
        return -EIO;
    }

    uint8_t buffer[WRITE_CHUNK_SIZE];
    uint32_t offset = 0;
    uint32_t total_written = 0;
    uint32_t last_log_offset = 0;

    while(offset < fw_size)
    {
        uint32_t chunk_size = MIN(WRITE_CHUNK_SIZE, fw_size - offset);

        ret = flash_area_read(fa, fw_offset + offset, buffer, chunk_size);
        if(ret < 0)
        {
            LOG_ERR("Partition okuma başarısız (offset: %u): %d", offset, ret);
            flash_area_close(fa);
            return ret;
        }

        int ret = stm32_write_memory(target_addr, buffer, chunk_size);
        if(ret < 0)
        {
            LOG_ERR("STM32 yazma başarısız (adres: 0x%08X): %d", target_addr, ret);
            flash_area_close(fa);
            return ret;
        }

        offset += chunk_size;
        target_addr += chunk_size;
        total_written += chunk_size;

        if(offset - last_log_offset >= 1024 * 5)
        {
            uint32_t percent = (offset * 100) / fw_size;
            LOG_INF("Yazıldı: %u%% (%u/%u bytes)", percent, offset, fw_size);
            last_log_offset = offset;
        }
    }

    flash_area_close(fa);
    LOG_INF("Firmware yazma tamamlandı! Toplam: %u bytes", total_written);
    return 0;
}

static int stm32_extended_erase_all(void)
{
    uart_poll_out(UART_DEV, 0x44);
    uart_poll_out(UART_DEV, 0xBB);
    int ret = wait_ack(500);
    if(ret < 0) return ret;
    uart_poll_out(UART_DEV, 0xFF);
    uart_poll_out(UART_DEV, 0xFF);
    uart_poll_out(UART_DEV, 0x00);
    ret = wait_ack(23000); // ~4.2sn
    if(ret < 0) return ret;
    return 0;
}

static void stm32_enter_bootloader(void)
{
    gpio_pin_set_dt(&boot0, 1);
    gpio_pin_set_dt(&nrst, 0);
    k_msleep(50);
    gpio_pin_set_dt(&nrst, 1);
    LOG_INF("STM32H7 bootloader moduna alındı (BOOT0=1 + reset)");
    k_msleep(300);
}

static void stm32_exit_bootloader(void)
{
    gpio_pin_set_dt(&boot0, 0);
    gpio_pin_set_dt(&nrst, 0);
    k_msleep(50);
    gpio_pin_set_dt(&nrst, 1);
    LOG_INF("STM32H7 bootloader modundan çıkarıldı. (BOOT0=0 + reset)");
    k_msleep(300);
}

static int stm32_bootloader_sync(void)
{
    uart_poll_out(UART_DEV, 0x7F);
    if(wait_ack(500) == 0)
    {
        return 0;
    }
    uart_poll_out(UART_DEV, 0x7F);
    if(wait_ack(500) == 0)
    {
        return 0;
    }
    uart_poll_out(UART_DEV, 0x7F);
    return wait_ack(500);        
}

int stm32_flashing_start(uint32_t fw_offset, uint32_t fw_size, uint32_t target_addr, bool boot_start)
{
    struct uart_config cfg =
    {
        .baudrate = 115200,
        .parity   = UART_CFG_PARITY_EVEN,
        .stop_bits = UART_CFG_STOP_BITS_1,
        .data_bits = UART_CFG_DATA_BITS_8,
        .flow_ctrl = UART_CFG_FLOW_CTRL_NONE,
    };

    int ret = uart_configure(uart_dev, &cfg);
    if(ret)
    {
        LOG_ERR("UART configure error: %d\n", ret);
    }

    LOG_WRN("ESP32-C6 → STM32H7 UART Flasher başladı");

    ret = gpio_pin_configure_dt(&boot0, GPIO_OUTPUT_HIGH);
    if(ret < 0)
    {
        LOG_ERR("BOOT0 configure failed: %d", ret);
        return ret;
    }

    ret = gpio_pin_configure_dt(&nrst, GPIO_OUTPUT_HIGH);
    if(ret < 0)
    {
        LOG_ERR("NRST configure failed: %d", ret);
        return ret;
    }

    if(boot_start)
    {
        stm32_enter_bootloader();
    }
    else
    {
        stm32_exit_bootloader();
    }

    ret = stm32_bootloader_sync();
    if(ret != 0)
    {
        LOG_ERR("Sync başarısız, kablolama/boot pinlerini kontrol et");
        stm32_exit_bootloader();
        return ret;
    }

    LOG_INF("Tüm Flashı Silme işlemi Başladı!");

    ret = stm32_extended_erase_all();
    if(ret < 0)
    {
        LOG_ERR("Flash silme başarısız!!! ret: %d", ret);
        stm32_exit_bootloader();
        return ret;
    }
    LOG_INF("Tüm Flash başarıyla silindi!");

    ret = stm32_write_firmware(fw_offset, fw_size, target_addr);
    if(ret < 0)
    {
        LOG_ERR("Firmware yazma başarısız!");
        stm32_exit_bootloader();
        return ret;
    }

    LOG_INF("Yazma işlemi bitti, doğrulama başlıyor...");
    ret = stm32_verify_firmware(fw_offset, fw_size, target_addr);
    if(ret < 0)
    {
        LOG_ERR("Doğrulama başarısız!");
        stm32_exit_bootloader();
        return ret;
    }

    stm32_exit_bootloader();
    LOG_WRN("=== ✓ Programlama Başarılı! ===");

    return 0;
}