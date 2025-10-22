// Copyright (c) 2025
// SPDX-License-Identifier: Apache-2.0
// Coskun ERGAN <coskunergan@gmail.com>

#include <zephyr/kernel.h>
#include <zephyr/canbus/isotp.h>
#include <zephyr/sys/util.h>
#include <zephyr/logging/log.h>

#include "mongoose_glue.h"
#include <zephyr/net/net_mgmt.h>
#include <zephyr/net/net_if.h>
#include <zephyr/net/wifi_mgmt.h>
#include <zephyr/net/dhcpv4_server.h>

LOG_MODULE_REGISTER(mg, LOG_LEVEL_DBG);

K_THREAD_STACK_DEFINE(mg_thread_stack, CONFIG_SAMPLE_MG_THREAD_STACK_SIZE);
struct k_thread mg_thread_data;

K_SEM_DEFINE(run, 0, 1);

#define CONFIG_WIFI_SAMPLE_SSID "SUPERONLINE_Wi-Fi_3AS4"
#define CONFIG_WIFI_SAMPLE_PSK "K7Q3SKzKC7QN"

static struct wifi_connect_req_params sta_config;

// https://docs.zephyrproject.org/latest/releases/migration-guide-4.2.html#networking
static void zeh(struct net_mgmt_event_callback *cb,
#if ZEPHYR_VERSION_CODE < 0x40200
                uint32_t mgmt_event,
#else
                uint64_t mgmt_event,
#endif
                struct net_if *iface)
{
    if(mgmt_event == NET_EVENT_L4_CONNECTED) k_sem_give(&run);
}

// Zephyr: Extract IP address when using DHCP
static void print_ipv4_address(void)
{
    struct net_if *iface = net_if_get_default();
    struct net_if_config *cfg = &iface->config;
    const struct net_if_addr *ifaddr = (const struct net_if_addr *)&cfg->ip.ipv4->unicast[0];
    MG_INFO(("IP: %M", mg_print_ip, &ifaddr->address.in_addr));
}

void mg_thread(void *arg1, void *arg2, void *arg3)
{
    ARG_UNUSED(arg1);
    ARG_UNUSED(arg2);
    ARG_UNUSED(arg3);

    struct net_mgmt_event_callback ncb;

    sta_config.ssid = (const uint8_t *)CONFIG_WIFI_SAMPLE_SSID;
    sta_config.ssid_length = sizeof(CONFIG_WIFI_SAMPLE_SSID) - 1;
    sta_config.psk = (const uint8_t *)CONFIG_WIFI_SAMPLE_PSK;
    sta_config.psk_length = sizeof(CONFIG_WIFI_SAMPLE_PSK) - 1;
    sta_config.security = WIFI_SECURITY_TYPE_PSK;
    sta_config.channel = WIFI_CHANNEL_ANY;
    sta_config.band = WIFI_FREQ_BAND_2_4_GHZ;

    LOG_INF("Connecting to SSID: %s\n", sta_config.ssid);

    int ret = net_mgmt(NET_REQUEST_WIFI_CONNECT, net_if_get_wifi_sta(), &sta_config,
                       sizeof(struct wifi_connect_req_params));

    k_msleep(5000);

    net_mgmt_init_event_callback(&ncb, zeh, NET_EVENT_L4_CONNECTED);
    net_mgmt_add_event_callback(&ncb);
    k_sem_take(&run, K_FOREVER);

    print_ipv4_address();

    k_msleep(5000);

    mongoose_init();

    for(;;)
    {
        mongoose_poll();
    }
}


int mg_init()
{
    k_tid_t tid = k_thread_create(&mg_thread_data, mg_thread_stack,
                                  K_THREAD_STACK_SIZEOF(mg_thread_stack),
                                  mg_thread, NULL, NULL, NULL,
                                  CONFIG_SAMPLE_MG_THREAD_PRIORITY, 0, K_NO_WAIT);
    if(!tid)
    {
        LOG_ERR("ERROR spawning mg thread\n");
        return -1;
    }
    k_thread_name_set(tid, "mg_thread");

    return 0;
}
