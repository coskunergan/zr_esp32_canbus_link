/*
 * Copyright (c) 2020 Gerson Fernando Budke <nandojve@gmail.com>
 *
 * SPDX-License-Identifier: Apache-2.0
 */
#include <zephyr/kernel.h>
#include <zephyr/logging/log.h>
#include <zephyr/net/wifi_mgmt.h>
#include <zephyr/net/dhcpv4_server.h>

LOG_MODULE_DECLARE(esp32_wifi, LOG_LEVEL_DBG);

#define MACSTR "%02X:%02X:%02X:%02X:%02X:%02X"

#define NET_EVENT_WIFI_MASK                                                                        \
	(NET_EVENT_WIFI_CONNECT_RESULT | NET_EVENT_WIFI_DISCONNECT_RESULT |                        \
	 NET_EVENT_WIFI_AP_ENABLE_RESULT | NET_EVENT_WIFI_AP_DISABLE_RESULT |                      \
	 NET_EVENT_WIFI_AP_STA_CONNECTED | NET_EVENT_WIFI_AP_STA_DISCONNECTED)

static struct net_if *ap_iface;
static struct net_if *sta_iface;

static struct wifi_connect_req_params ap_config;
static struct wifi_connect_req_params sta_config;

static bool connected;

static struct net_mgmt_event_callback cb;

extern uint8_t get_current_ssid_len(void);
extern const uint8_t *get_current_ssid(void);
extern uint8_t get_current_psk_len(void);
extern const uint8_t *get_current_psk(void);

/* Check necessary definitions */

#if WIFI_SAMPLE_DHCPV4_START
BUILD_ASSERT(sizeof(CONFIG_WIFI_SAMPLE_AP_IP_ADDRESS) > 1,
             "CONFIG_WIFI_SAMPLE_AP_IP_ADDRESS is empty. Please set it in conf file.");

BUILD_ASSERT(sizeof(CONFIG_WIFI_SAMPLE_AP_NETMASK) > 1,
             "CONFIG_WIFI_SAMPLE_AP_NETMASK is empty. Please set it in conf file.");

#endif

static void wifi_event_handler(struct net_mgmt_event_callback *cb, uint64_t mgmt_event,
                               struct net_if *iface)
{
    switch(mgmt_event)
    {
        case NET_EVENT_WIFI_CONNECT_RESULT:
        {
            const struct wifi_status *status = (const struct wifi_status *)cb->info;
            if(status->status == 0)
            {
                LOG_INF("WiFi connected successfully! to %s", get_current_ssid());
                connected = true;
            }
            else
            {
                LOG_ERR("WiFi connection failed, status code: %d", status->status);
                connected = false;
            }
            break;
        }
        case NET_EVENT_WIFI_DISCONNECT_RESULT:
        {
            connected = false;
            LOG_INF("Disconnected from %s", get_current_ssid());
            break;
        }
        case NET_EVENT_WIFI_AP_ENABLE_RESULT:
        {
            LOG_INF("AP Mode is enabled. Waiting for station to connect");
            break;
        }
        case NET_EVENT_WIFI_AP_DISABLE_RESULT:
        {
            LOG_INF("AP Mode is disabled.");
            break;
        }
        case NET_EVENT_WIFI_AP_STA_CONNECTED:
        {
            struct wifi_ap_sta_info *sta_info = (struct wifi_ap_sta_info *)cb->info;

            LOG_INF("station: " MACSTR " joined ", sta_info->mac[0], sta_info->mac[1],
                    sta_info->mac[2], sta_info->mac[3], sta_info->mac[4], sta_info->mac[5]);
            break;
        }
        case NET_EVENT_WIFI_AP_STA_DISCONNECTED:
        {
            struct wifi_ap_sta_info *sta_info = (struct wifi_ap_sta_info *)cb->info;

            LOG_INF("station: " MACSTR " leave ", sta_info->mac[0], sta_info->mac[1],
                    sta_info->mac[2], sta_info->mac[3], sta_info->mac[4], sta_info->mac[5]);
            break;
        }
        default:
            break;
    }
}


#if CONFIG_WIFI_SAMPLE_DHCPV4_START
static void enable_dhcpv4_server(void)
{
    static struct in_addr addr;
    static struct in_addr netmaskAddr;

    if(net_addr_pton(AF_INET, CONFIG_WIFI_SAMPLE_AP_IP_ADDRESS, &addr))
    {
        LOG_ERR("Invalid address: %s", CONFIG_WIFI_SAMPLE_AP_IP_ADDRESS);
        return;
    }

    if(net_addr_pton(AF_INET, CONFIG_WIFI_SAMPLE_AP_NETMASK, &netmaskAddr))
    {
        LOG_ERR("Invalid netmask: %s", CONFIG_WIFI_SAMPLE_AP_NETMASK);
        return;
    }

    net_if_ipv4_set_gw(ap_iface, &addr);

    if(net_if_ipv4_addr_add(ap_iface, &addr, NET_ADDR_MANUAL, 0) == NULL)
    {
        LOG_ERR("unable to set IP address for AP interface");
    }

    if(!net_if_ipv4_set_netmask_by_addr(ap_iface, &addr, &netmaskAddr))
    {
        LOG_ERR("Unable to set netmask for AP interface: %s",
                CONFIG_WIFI_SAMPLE_AP_NETMASK);
    }

    addr.s4_addr[3] += 10; /* Starting IPv4 address for DHCPv4 address pool. */

    if(net_dhcpv4_server_start(ap_iface, &addr) != 0)
    {
        LOG_ERR("DHCP server is not started for desired IP");
        return;
    }

    LOG_INF("DHCPv4 server started...\n");
}
#endif

static int enable_ap_mode(void)
{
    if(!ap_iface)
    {
        LOG_INF("AP: is not initialized");
        return -EIO;
    }

    // struct in_addr ipv4_addr;
    // struct in_addr netmask;
    // struct in_addr gateway;

    // if(net_addr_pton(AF_INET, CONFIG_NET_CONFIG_MY_IPV4_ADDR, &ipv4_addr))
    // {
    //     LOG_ERR("Hata: Geçersiz IP adresi");
    //     return -EINVAL;
    // }
    // if(net_addr_pton(AF_INET, CONFIG_NET_CONFIG_MY_IPV4_NETMASK, &netmask))
    // {
    //     LOG_ERR("Hata: Geçersiz Ağ Maskesi");
    //     return -EINVAL;
    // }
    // if(net_addr_pton(AF_INET, CONFIG_NET_CONFIG_MY_IPV4_GW, &gateway))
    // {
    //     LOG_ERR("Hata: Geçersiz Ağ Geçidi");
    //     return -EINVAL;
    // }

    LOG_INF("Turning on AP Mode");
    ap_config.ssid = (const uint8_t *)CONFIG_WIFI_SAMPLE_AP_SSID;
    ap_config.ssid_length = sizeof(CONFIG_WIFI_SAMPLE_AP_SSID) - 1;
    ap_config.psk = (const uint8_t *)CONFIG_WIFI_SAMPLE_AP_PSK;
    ap_config.psk_length = sizeof(CONFIG_WIFI_SAMPLE_AP_PSK) - 1;
    ap_config.channel = WIFI_CHANNEL_ANY;
    ap_config.band = WIFI_FREQ_BAND_2_4_GHZ;

    if(sizeof(CONFIG_WIFI_SAMPLE_AP_PSK) == 1)
    {
        ap_config.security = WIFI_SECURITY_TYPE_NONE;
    }
    else
    {
        ap_config.security = WIFI_SECURITY_TYPE_PSK;
    }

    // struct in_addr src = { { { 192, 168, 0, 1 } } };

    // net_if_ipv4_addr_add(ap_iface, &ipv4_addr, NET_ADDR_MANUAL, 0);
    
    // net_if_ipv4_set_netmask_by_addr(ap_iface, &ipv4_addr, &netmask);

    // net_if_ipv4_set_gw(ap_iface, &gateway);

#if CONFIG_WIFI_SAMPLE_DHCPV4_START
    enable_dhcpv4_server();
#endif

    int ret = net_mgmt(NET_REQUEST_WIFI_AP_ENABLE, ap_iface, &ap_config,
                       sizeof(struct wifi_connect_req_params));
    if(ret)
    {
        LOG_ERR("NET_REQUEST_WIFI_AP_ENABLE failed, err: %d", ret);
    }

    return ret;
}

static int connect_to_wifi(void)
{
    if(!sta_iface)
    {
        LOG_INF("STA: interface no initialized");
        return -EIO;
    }

    struct in_addr ipv4_addr;
    struct in_addr netmask;
    struct in_addr gateway;

    if(net_addr_pton(AF_INET, CONFIG_NET_CONFIG_MY_IPV4_ADDR, &ipv4_addr))
    {
        LOG_ERR("Hata: Geçersiz IP adresi");
        return -EINVAL;
    }
    if(net_addr_pton(AF_INET, CONFIG_NET_CONFIG_MY_IPV4_NETMASK, &netmask))
    {
        LOG_ERR("Hata: Geçersiz Ağ Maskesi");
        return -EINVAL;
    }
    if(net_addr_pton(AF_INET, CONFIG_NET_CONFIG_MY_IPV4_GW, &gateway))
    {
        LOG_ERR("Hata: Geçersiz Ağ Geçidi");
        return -EINVAL;
    }
    sta_config.ssid = get_current_ssid();
    sta_config.ssid_length = get_current_ssid_len();
    sta_config.psk = get_current_psk();
    sta_config.psk_length = get_current_psk_len();
    sta_config.security = WIFI_SECURITY_TYPE_PSK;
    sta_config.channel = WIFI_CHANNEL_ANY;
    sta_config.band = WIFI_FREQ_BAND_2_4_GHZ;

    net_if_ipv4_addr_add(sta_iface, &ipv4_addr, NET_ADDR_MANUAL, 0);

    net_if_ipv4_set_gw(sta_iface, &gateway);

    LOG_INF("Connecting to SSID: %s\n", sta_config.ssid);

    int retries = 8;
    int ret;

    connected = false;

    while(retries--)
    {
        LOG_INF("WiFi connect attempt %d...", 8 - retries);

        ret = net_mgmt(NET_REQUEST_WIFI_CONNECT, sta_iface,
                       &sta_config, sizeof(sta_config));

        if(ret != 0)
        {
            LOG_ERR("net_mgmt() failed: %d", ret);
            k_sleep(K_MSEC(2000));
            continue;
        }

        int timeout = 40;  // 20 saniye (500 ms x 40)
        while(timeout-- && !connected)
        {
            k_msleep(500);
        }

        if(connected)
        {
            LOG_INF("WiFi successfully connected with static IP!");
            return 0;
        }

        LOG_WRN("Connection failed or timed out, retrying in 3s...");
        k_sleep(K_MSEC(3000));
    }
    LOG_ERR("All WiFi connection attempts failed!");

    return -EINVAL;
}


void wifi_connect(void)
{
    net_mgmt_init_event_callback(&cb, wifi_event_handler, NET_EVENT_WIFI_MASK);
    net_mgmt_add_event_callback(&cb);

    ap_iface = net_if_get_wifi_sap();

    sta_iface = net_if_get_wifi_sta();

    enable_ap_mode();
    //connect_to_wifi();
}

