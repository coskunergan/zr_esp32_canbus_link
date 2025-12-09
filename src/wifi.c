/*
 * Copyright (c) 2020 Gerson Fernando Budke <nandojve@gmail.com>
 *
 * SPDX-License-Identifier: Apache-2.0
 */
#include <zephyr/kernel.h>
#include <zephyr/logging/log.h>
#include <zephyr/net/wifi_mgmt.h>
#include <zephyr/net/dhcpv4_server.h>
#include <zephyr/net/dhcpv4.h>
#include <zephyr/net/net_if.h>
#include <zephyr/net/icmp.h>
#include <zephyr/net/socket.h>
#include <zephyr/net/net_core.h> 
#include <string.h>
#include <zephyr/net/dhcpv4.h> 
#include <errno.h> 

LOG_MODULE_DECLARE(esp32_wifi, LOG_LEVEL_DBG);

// Temporary stubs until Rust implementation
__weak int rust_nat_init(void) { return 0; }
__weak int rust_nat_outbound(struct net_pkt *pkt) { return 0; }
__weak int rust_nat_inbound(struct net_pkt *pkt) { return 0; }

#define MACSTR "%02X:%02X:%02X:%02X:%02X:%02X"

// IPV4_ADDR_ADD olayı artık kullanılmıyor.
#define NET_EVENT_WIFI_MASK                                                                        \
	(NET_EVENT_WIFI_CONNECT_RESULT | NET_EVENT_WIFI_DISCONNECT_RESULT |                        \
	 NET_EVENT_WIFI_AP_ENABLE_RESULT | NET_EVENT_WIFI_AP_DISABLE_RESULT |                      \
	 NET_EVENT_WIFI_AP_STA_CONNECTED | NET_EVENT_WIFI_AP_STA_DISCONNECTED)

static struct in_addr ap_current_ip = { .s4_addr = {0} };

static struct wifi_connect_req_params ap_config;
static struct wifi_connect_req_params sta_config;

static bool connected;
static bool sta_ip_assigned = false;

static struct net_mgmt_event_callback cb;

/* Global interface pointers */
struct net_if *ap_iface = NULL;
struct net_if *sta_iface = NULL;
uint32_t sta_ip_addr = 0; 

/* Asenkron IP atama için işleyici yapısı */
static struct k_work_delayable ip_config_work; 

// Harici fonksiyon bildirimleri
extern uint8_t get_current_ssid_len(void);
extern const uint8_t *get_current_ssid(void);
extern uint8_t get_current_psk_len(void);
extern const uint8_t *get_current_psk(void);

#if defined(CONFIG_NET_IPV4_FORWARDING)
void setup_nat_simple(void)
{
    printk("[NAT-SIMPLE] Setting up NAT\n");

    if(!ap_iface || !sta_iface)
    {
        printk("[NAT-SIMPLE-ERROR] Interfaces not initialized!\n");
        return;
    }

    /* 1. AP interface için gateway kendisi olsun */
    struct in_addr ap_gateway = { .s4_addr = {192, 168, 4, 1} };
    net_if_ipv4_set_gw(ap_iface, &ap_gateway);

    /* 2. STA interface için doğru gateway (ev router'ı) */
    struct in_addr sta_gateway = { .s4_addr = {192, 168, 1, 1} };
    net_if_ipv4_set_gw(sta_iface, &sta_gateway);

    printk("[NAT-SIMPLE] NAT configuration complete\n");
}
#endif

#if CONFIG_NET_DHCPV4_SERVER
static void enable_dhcpv4_server(void)
{
    if(!ap_iface)
    {
        LOG_ERR("AP interface is NULL!");
        return;
    }

    if(!net_if_is_up(ap_iface))
    {
        net_if_up(ap_iface);
        k_sleep(K_MSEC(300));
    }

    struct in_addr ap_ip = { .s4_addr = {192, 168, 4, 1} };  
    struct in_addr netmask = { .s4_addr = {255, 255, 255, 0} }; // GERİ DÖNÜŞ
    struct in_addr gateway = { .s4_addr = {192, 168, 4, 1} }; 

    net_if_ipv4_addr_rm(ap_iface, &ap_ip);
    net_if_ipv4_addr_add(ap_iface, &ap_ip, NET_ADDR_MANUAL, 0);
    
    // DEPRECATED fonksiyona geri döndük (Linker hatasını önlemek için)
    net_if_ipv4_set_netmask(ap_iface, &netmask); 
    
    net_if_ipv4_set_gw(ap_iface, &gateway);

    ap_current_ip = ap_ip;

    LOG_INF("AP configured → 192.168.4.1/24 | Gateway: 192.168.4.1");

    struct in_addr pool_start = ap_ip;
    pool_start.s4_addr[3] = 100;

    if(net_dhcpv4_server_start(ap_iface, &pool_start) != 0)
    {
        LOG_ERR("DHCP server start failed");
        return;
    }

    LOG_INF("DHCP server STARTED → 192.168.4.100+");
}
#endif

/* Router bağlantı testi - SOCKET KULLAN */
static void test_router_connectivity_socket(void)
{
    if(!sta_iface || sta_ip_addr == 0)
    {
        printk("[ROUTER-TEST-ERROR] STA interface not ready or IP not assigned\n");
        return;
    }

    printk("\n[ROUTER-TEST] Testing router connectivity with SOCKET...\n");

    int sock = zsock_socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
    if(sock < 0)
    {
        printk("[ROUTER-TEST-ERROR] Cannot create socket: %d (errno: %d)\n", sock, errno);
        return;
    }

    struct sockaddr_in local_addr =
    {
        .sin_family = AF_INET,
        .sin_port = htons(0),  
        .sin_addr = { .s_addr = htonl(sta_ip_addr) } 
    };

    if(zsock_bind(sock, (struct sockaddr *)&local_addr, sizeof(local_addr)) < 0)
    {
        printk("[ROUTER-TEST-ERROR] Cannot bind socket (errno: %d)\n", errno);
        zsock_close(sock);
        return;
    }

    printk("[ROUTER-TEST] Socket created and bound to %u.%u.%u.%u\n",
           (uint8_t)(sta_ip_addr >> 24), (uint8_t)(sta_ip_addr >> 16),
           (uint8_t)(sta_ip_addr >> 8), (uint8_t)sta_ip_addr);

    /* Router'a UDP paketi göndermeyi dene (192.168.1.1:53) */
    struct sockaddr_in router_addr =
    {
        .sin_family = AF_INET,
        .sin_port = htons(53),  
        .sin_addr = { .s_addr = htonl(0xC0A80101) } 
    };

    char test_packet[] = "TEST";
    int send_result = zsock_sendto(sock, test_packet, sizeof(test_packet), 0,
                             (struct sockaddr *)&router_addr, sizeof(router_addr));

    if(send_result < 0)
    {
        printk("[ROUTER-TEST-ERROR] Cannot send to router (errno: %d)\n", errno);
    }
    else
    {
        printk("[ROUTER-TEST-SUCCESS] Packet sent to router! Send result: %d\n", send_result);

        /* İnternet testi (8.8.8.8:53) */
        struct sockaddr_in google_dns =
        {
            .sin_family = AF_INET,
            .sin_port = htons(53),
            .sin_addr = { .s_addr = htonl(0x08080808) } 
        };

        send_result = zsock_sendto(sock, test_packet, sizeof(test_packet), 0,
                             (struct sockaddr *)&google_dns, sizeof(google_dns));

        if(send_result < 0)
        {
            printk("[INTERNET-TEST] Cannot reach internet (expected if NAT not working/route missing) (errno: %d)\n", errno);
        }
        else
        {
            printk("[INTERNET-TEST-SUCCESS] Can reach internet! Send result: %d\n", send_result);
        }
    }

    zsock_close(sock);
    printk("[ROUTER-TEST] Socket test completed\n");
}

/* Interface durumunu kontrol et */
static void check_interface_status(void)
{
    printk("\n[IF-STATUS] Checking interface status...\n");

    if(sta_iface)
    {
        printk("[IF-STATUS] STA Interface: 0x%p\n", (void *)sta_iface);
        printk("[IF-STATUS] STA is UP: %s\n", net_if_is_up(sta_iface) ? "YES" : "NO");
        printk("[IF-STATUS] STA is LINKED: %s\n", net_if_is_carrier_ok(sta_iface) ? "YES" : "NO");

        if(sta_ip_assigned)
        {
            printk("[IF-STATUS] STA IPv4 configured: YES\n");
        }
        else
        {
            printk("[IF-STATUS] STA IPv4 NOT configured (Manuel IP bekleniyor).!\n");
        }

        struct net_if_ipv4 *ipv4 = sta_iface->config.ip.ipv4;
        if(ipv4)
        {
            printk("[IF-STATUS] STA Gateway: %d.%d.%d.%d\n",
                   ipv4->gw.s4_addr[0], ipv4->gw.s4_addr[1],
                   ipv4->gw.s4_addr[2], ipv4->gw.s4_addr[3]);
        }
    }

    if(ap_iface)
    {
        printk("[IF-STATUS] AP Interface: 0x%p\n", (void *)ap_iface);
        printk("[IF-STATUS] AP is UP: %s\n", net_if_is_up(ap_iface) ? "YES" : "NO");
        printk("[IF-STATUS] AP is LINKED: %s\n", net_if_is_carrier_ok(ap_iface) ? "YES" : "NO");
    }
}

static int configure_sta_manual_ip(void)
{
    if(!sta_iface)
    {
        printk("[STA-IP] HATA: STA interface NULL!\n");
        return -EIO;
    }

    printk("[STA-IP] 1. Basamak: MANUAL IP yapilandirmasi basliyor...\n");

    struct in_addr sta_ip = { .s4_addr = {192, 168, 1, 77} };
    struct in_addr netmask = { .s4_addr = {255, 255, 255, 0} }; // GERİ DÖNÜŞ
    struct in_addr gateway = { .s4_addr = {192, 168, 1, 1} };

    net_if_ipv4_addr_rm(sta_iface, &sta_ip);

    if(net_if_ipv4_addr_add(sta_iface, &sta_ip, NET_ADDR_MANUAL, 0) == NULL)
    {
        printk("[STA-IP] HATA: Statik IP atamasi basarisiz oldu. Arayuz durumu: %s (errno: %d)\n",
               net_if_is_up(sta_iface) ? "UP" : "DOWN", errno);
        return -EIO;
    }

    printk("[STA-IP] 2. Basamak: IP adresi basariyla eklendi.\n");

    // DEPRECATED fonksiyona geri döndük (Linker hatasını önlemek için)
    net_if_ipv4_set_netmask(sta_iface, &netmask);
    
    net_if_ipv4_set_gw(sta_iface, &gateway);

    sta_ip_addr = ntohl(sta_ip.s_addr);
    sta_ip_assigned = true;

    printk("[STA-IP] 3. Basamak: MANUAL IP yapilandirildi:\n");
    printk("[STA-IP]   IP:      %u.%u.%u.%u\n",
           sta_ip.s4_addr[0], sta_ip.s4_addr[1],
           sta_ip.s4_addr[2], sta_ip.s4_addr[3]);
    printk("[STA-IP]   Gateway: %u.%u.%u.%u\n",
           gateway.s4_addr[0], gateway.s4_addr[1],
           gateway.s4_addr[2], gateway.s4_addr[3]);

    return 0;
}

/* Baglantidan 3 saniye sonra calisan isleyici. */
static void ip_config_work_handler(struct k_work *work)
{
    if (!sta_iface) {
        printk(">>> HATA: ip_config_work_handler sta_iface NULL!\n");
        return;
    }

    printk(">>> ASENKRON ADIM 2: IP konfigürasyonu basliyor (Gecikmeli calisma)...\n");
    
    net_dhcpv4_stop(sta_iface); 
    
    configure_sta_manual_ip(); 
    
    check_interface_status();
    
    #if defined(CONFIG_NET_IPV4_FORWARDING)
    setup_nat_simple();
    #endif
    
    // k_sleep(K_MSEC(2000));
    // test_router_connectivity_socket();
}

// Olay işleyici imzası korundu, uyarıya rağmen linker hatası vermez.
static void wifi_event_handler(struct net_mgmt_event_callback *cb, uint64_t mgmt_event,
                               struct net_if *iface)
{
    switch(mgmt_event)
    {
        case NET_EVENT_WIFI_CONNECT_RESULT:
        {
            const struct wifi_status *status = (const struct wifi_status *)cb->info;

            if(status->status != 0)
            {
                printk(">>> HATA: WiFi baglanti hatasi: %d\n", status->status); 
                connected = false;
                break;
            }

            sta_iface = iface; 

            printk(">>> ADIM 1: WiFi baglandi! Asenkron IP atama 3 saniye sonra tetikleniyor...\n");
            connected = true;
            
            net_if_up(iface);
            
            net_dhcpv4_stop(iface); 

            k_work_init_delayable(&ip_config_work, ip_config_work_handler);
            k_work_schedule(&ip_config_work, K_MSEC(3000)); 
            
            break;
        }

        case NET_EVENT_WIFI_DISCONNECT_RESULT:
        {
            connected = false;
            sta_ip_assigned = false;
            k_work_cancel_delayable(&ip_config_work); 
            net_dhcpv4_stop(iface);
            LOG_INF("Disconnected from %s", get_current_ssid());
            break;
        }
        case NET_EVENT_WIFI_AP_STA_CONNECTED:
        {
            struct wifi_ap_sta_info *sta_info = (struct wifi_ap_sta_info *)cb->info;

            LOG_INF("station: " MACSTR " joined ", sta_info->mac[0], sta_info->mac[1],
                    sta_info->mac[2], sta_info->mac[3], sta_info->mac[4], sta_info->mac[5]);

            if(sta_ip_assigned)
            {
                k_sleep(K_MSEC(1000));
                test_router_connectivity_socket();
            }

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

static int enable_ap_mode(void)
{
    if(!ap_iface)
    {
        LOG_INF("AP: is not initialized");
        return -EIO;
    }

    LOG_INF("Turning on AP Mode");
    ap_config.ssid = (const uint8_t *)CONFIG_WIFI_SAMPLE_AP_SSID;
    ap_config.ssid_length = sizeof(CONFIG_WIFI_SAMPLE_AP_SSID) - 1;
    ap_config.psk = (const uint8_t *)CONFIG_WIFI_SAMPLE_AP_PSK;
    
    // Güvenlik tipini PSK uzunluğuna göre ayarla
    if (sizeof(CONFIG_WIFI_SAMPLE_AP_PSK) <= 1) {
        ap_config.security = WIFI_SECURITY_TYPE_NONE;
        ap_config.psk_length = 0;
    } else {
        ap_config.security = WIFI_SECURITY_TYPE_PSK;
        ap_config.psk_length = sizeof(CONFIG_WIFI_SAMPLE_AP_PSK) - 1;
    }

    ap_config.channel = WIFI_CHANNEL_ANY;
    ap_config.band = WIFI_FREQ_BAND_2_4_GHZ;


#if CONFIG_NET_DHCPV4_SERVER
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
        LOG_INF("STA: interface not initialized");
        return -EIO;
    }

    sta_config.ssid = get_current_ssid();
    sta_config.ssid_length = get_current_ssid_len();
    sta_config.psk = get_current_psk();
    sta_config.psk_length = get_current_psk_len();
    sta_config.channel = WIFI_CHANNEL_ANY;
    sta_config.band = WIFI_FREQ_BAND_2_4_GHZ;

    // KRİTİK DÜZELTME: PSK uzunluğuna göre güvenlik tipini doğru ayarla.
    if (sta_config.psk_length > 0) {
        sta_config.security = WIFI_SECURITY_TYPE_PSK;
    } else {
        sta_config.security = WIFI_SECURITY_TYPE_NONE;
    }

    LOG_INF("Connecting to SSID: %s (PSK Len: %d, Security: %s)", 
            sta_config.ssid, sta_config.psk_length, 
            sta_config.security == WIFI_SECURITY_TYPE_PSK ? "PSK" : "NONE");


    int retries = 3;
    int ret;

    connected = false;
    sta_ip_assigned = false;

    while(retries--)
    {
        LOG_INF("WiFi connect attempt %d...", 2 - retries);

        ret = net_mgmt(NET_REQUEST_WIFI_CONNECT, sta_iface,
                       &sta_config, sizeof(sta_config));

        if(ret != 0)
        {
            if(ret == -EALREADY)
            {
                LOG_INF("WiFi already connected or connecting");
                connected = true;
                break;
            }
            else
            {
                LOG_ERR("net_mgmt() failed: %d", ret);
                k_sleep(K_MSEC(3000));
                continue;
            }
        }

        /* Bağlantı için bekle */
        int timeout = 30;
        while(timeout-- && !connected)
        {
            k_msleep(500);
        }

        if(connected)
        {
            LOG_INF("WiFi connected!");
            break;
        }

        LOG_WRN("Connection timeout, retrying...");
    }

    if(!connected)
    {
        LOG_ERR("WiFi connection failed!");
        return -EIO;
    }

    return 0;
}

void wifi_connect(void)
{
    k_sleep(K_MSEC(1000));

    net_mgmt_init_event_callback(&cb, wifi_event_handler, NET_EVENT_WIFI_MASK);
    net_mgmt_add_event_callback(&cb);

    ap_iface = net_if_get_wifi_sap();
    sta_iface = net_if_get_wifi_sta();

    if(!ap_iface || !sta_iface)
    {
        LOG_ERR("AP veya STA interface bulunamadı!");
        return;
    }

    LOG_INF("AP Interface: 0x%p, STA Interface: 0x%p",
            (void *)ap_iface, (void *)sta_iface);

    if(enable_ap_mode() != 0)
    {
        LOG_ERR("AP mode activation failed!");
        return;
    }

    if(connect_to_wifi() == 0)
    {
        check_interface_status(); 
        printk("[WIFI] Manuel IP ataması Asenkron Görev ile başlatıldı. 3 saniye sonra kontrol edin.\n");
        LOG_INF("\n=== TROUBLESHOOTING GUIDE ===\n");
    }
    else
    {
        LOG_ERR("WiFi STA connection failed!");
    }
}