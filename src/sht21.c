// Copyright (c) 2025
// SPDX-License-Identifier: Apache-2.0
// Coskun ERGAN <coskunergan@gmail.com>

#include <zephyr/kernel.h>
#include <zephyr/devicetree.h>
#include <zephyr/device.h>
#include <zephyr/drivers/sensor.h>
#include <zephyr/logging/log.h>

LOG_MODULE_REGISTER(sht21, LOG_LEVEL_DBG);

static const struct device *const sht21_dev = DEVICE_DT_GET_OR_NULL(DT_NODELABEL(sht21));

int sht21_init()
{
    if(!device_is_ready(sht21_dev))
    {
        LOG_ERR("SHT21 device is not ready.");
        return -ENODEV;
    }

    return 0;
}

int sht21_read_data(float *temperature, float *humidity)
{
    struct sensor_value temp_val, hum_val;
    int ret;

    if(sht21_dev == NULL)
    {
        LOG_ERR("HATA: Sensör başlatılmamış!\n");
        return -EINVAL;
    }

    if(temperature == NULL || humidity == NULL)
    {
        LOG_ERR("HATA: Geçersiz pointer!\n");
        return -EINVAL;
    }

    ret = sensor_sample_fetch(sht21_dev);
    if(ret)
    {
        LOG_ERR("HATA: Sensor sample fetch hatası: %d\n", ret);
        return ret;
    }

    ret = sensor_channel_get(sht21_dev, SENSOR_CHAN_AMBIENT_TEMP, &temp_val);
    if(ret)
    {
        LOG_ERR("HATA: Sıcaklık okuma hatası: %d\n", ret);
        return ret;
    }

    ret = sensor_channel_get(sht21_dev, SENSOR_CHAN_HUMIDITY, &hum_val);
    if(ret)
    {
        LOG_ERR("HATA: Nem okuma hatası: %d\n", ret);
        return ret;
    }

    *temperature = sensor_value_to_double(&temp_val);
    *humidity = sensor_value_to_double(&hum_val);

    return 0;
}

int sht21_read_temperature(float *temperature)
{
    struct sensor_value temp_val;
    int ret;

    if(sht21_dev == NULL)
    {
        return -EINVAL;
    }

    if(temperature == NULL)
    {
        return -EINVAL;
    }

    ret = sensor_sample_fetch_chan(sht21_dev, SENSOR_CHAN_AMBIENT_TEMP);
    if(ret)
    {
        return ret;
    }

    ret = sensor_channel_get(sht21_dev, SENSOR_CHAN_AMBIENT_TEMP, &temp_val);
    if(ret)
    {
        return ret;
    }

    *temperature = sensor_value_to_double(&temp_val);
    return 0;
}

int sht21_read_humidity(float *humidity)
{
    struct sensor_value hum_val;
    int ret;

    if(sht21_dev == NULL)
    {
        return -EINVAL;
    }

    if(humidity == NULL)
    {
        return -EINVAL;
    }

    ret = sensor_sample_fetch_chan(sht21_dev, SENSOR_CHAN_HUMIDITY);
    if(ret)
    {
        return ret;
    }

    ret = sensor_channel_get(sht21_dev, SENSOR_CHAN_HUMIDITY, &hum_val);
    if(ret)
    {
        return ret;
    }

    *humidity = sensor_value_to_double(&hum_val);
    return 0;
}

bool sht21_is_ready()
{
    if(sht21_dev == NULL)
    {
        return false;
    }
    return device_is_ready(sht21_dev);
}
