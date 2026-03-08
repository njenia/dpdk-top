/*
 * minimal_dpdk_app — A tiny DPDK application that demonstrates using DPDK
 * as a library. Initializes EAL, opens one port, creates a mempool, sets up
 * RX/TX queues, and polls for packets in a simple rx-only loop.
 *
 * Telemetry is enabled by default in DPDK 23+, so dpdk-top can connect to
 * this application's telemetry socket just like it would to testpmd or l3fwd.
 *
 * Usage:
 *   sudo ./minimal_dpdk_app -l 0-1 --file-prefix=myapp -a 0000:00:08.0 -- --rxq 4
 */

#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include <rte_eal.h>
#include <rte_ethdev.h>
#include <rte_mbuf.h>
#include <rte_mempool.h>

#define RX_RING_SIZE 1024
#define TX_RING_SIZE 1024
#define NUM_MBUFS    16384
#define MBUF_CACHE   256
#define BURST_SIZE   32

static volatile int keep_running = 1;

static void signal_handler(int sig)
{
    (void)sig;
    keep_running = 0;
}

static int port_init(uint16_t port, struct rte_mempool *pool, uint16_t nb_rxq, uint16_t nb_txq)
{
    struct rte_eth_conf port_conf;
    struct rte_eth_dev_info dev_info;
    int ret;

    memset(&port_conf, 0, sizeof(port_conf));

    ret = rte_eth_dev_info_get(port, &dev_info);
    if (ret != 0) {
        fprintf(stderr, "Error getting dev info for port %u: %s\n",
                port, rte_strerror(-ret));
        return ret;
    }

    /* Cap queues to what the device supports */
    if (nb_rxq > dev_info.max_rx_queues)
        nb_rxq = dev_info.max_rx_queues;
    if (nb_txq > dev_info.max_tx_queues)
        nb_txq = dev_info.max_tx_queues;

    /* Enable RSS if multiple RX queues */
    if (nb_rxq > 1) {
        port_conf.rxmode.mq_mode = RTE_ETH_MQ_RX_RSS;
        port_conf.rx_adv_conf.rss_conf.rss_key = NULL; /* use default key */
        port_conf.rx_adv_conf.rss_conf.rss_hf =
            (RTE_ETH_RSS_IP | RTE_ETH_RSS_UDP | RTE_ETH_RSS_TCP) &
            dev_info.flow_type_rss_offloads;
    }

    ret = rte_eth_dev_configure(port, nb_rxq, nb_txq, &port_conf);
    if (ret != 0) {
        fprintf(stderr, "Error configuring port %u: %s\n",
                port, rte_strerror(-ret));
        return ret;
    }

    uint16_t adjusted_rx = nb_rxq, adjusted_tx = nb_txq;
    ret = rte_eth_dev_adjust_nb_rx_tx_desc(port, &adjusted_rx, &adjusted_tx);
    if (ret != 0)
        fprintf(stderr, "Warning: adjust desc failed: %s\n", rte_strerror(-ret));

    for (uint16_t q = 0; q < nb_rxq; q++) {
        ret = rte_eth_rx_queue_setup(port, q, RX_RING_SIZE,
                                      rte_eth_dev_socket_id(port), NULL, pool);
        if (ret < 0) {
            fprintf(stderr, "RX queue %u setup failed: %s\n", q, rte_strerror(-ret));
            return ret;
        }
    }

    for (uint16_t q = 0; q < nb_txq; q++) {
        ret = rte_eth_tx_queue_setup(port, q, TX_RING_SIZE,
                                      rte_eth_dev_socket_id(port), NULL);
        if (ret < 0) {
            fprintf(stderr, "TX queue %u setup failed: %s\n", q, rte_strerror(-ret));
            return ret;
        }
    }

    ret = rte_eth_dev_start(port);
    if (ret < 0) {
        fprintf(stderr, "Error starting port %u: %s\n",
                port, rte_strerror(-ret));
        return ret;
    }

    struct rte_ether_addr addr;
    ret = rte_eth_macaddr_get(port, &addr);
    if (ret == 0) {
        printf("Port %u MAC: %02x:%02x:%02x:%02x:%02x:%02x\n", port,
               addr.addr_bytes[0], addr.addr_bytes[1], addr.addr_bytes[2],
               addr.addr_bytes[3], addr.addr_bytes[4], addr.addr_bytes[5]);
    }

    printf("Port %u started: %u RX queues, %u TX queues\n", port, nb_rxq, nb_txq);
    return 0;
}

int main(int argc, char *argv[])
{
    int ret;
    uint16_t nb_rxq = 1;
    uint16_t nb_txq = 1;
    uint16_t port_id;
    uint16_t nb_ports;

    /* Initialize EAL — this also starts the telemetry thread */
    ret = rte_eal_init(argc, argv);
    if (ret < 0)
        rte_exit(EXIT_FAILURE, "EAL init failed\n");

    argc -= ret;
    argv += ret;

    /* Parse app-specific args: --rxq N */
    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "--rxq") == 0 && i + 1 < argc) {
            nb_rxq = (uint16_t)atoi(argv[++i]);
            nb_txq = nb_rxq;
        }
    }

    signal(SIGINT, signal_handler);
    signal(SIGTERM, signal_handler);

    nb_ports = rte_eth_dev_count_avail();
    if (nb_ports == 0)
        rte_exit(EXIT_FAILURE, "No Ethernet ports found\n");

    printf("Found %u DPDK port(s)\n", nb_ports);

    /* Create a single shared mempool */
    struct rte_mempool *pool = rte_pktmbuf_pool_create(
        "app_mbuf_pool", NUM_MBUFS * nb_ports, MBUF_CACHE, 0,
        RTE_MBUF_DEFAULT_BUF_SIZE, rte_socket_id());
    if (pool == NULL)
        rte_exit(EXIT_FAILURE, "Cannot create mbuf pool: %s\n",
                 rte_strerror(rte_errno));

    /* Initialize all available ports */
    RTE_ETH_FOREACH_DEV(port_id) {
        ret = port_init(port_id, pool, nb_rxq, nb_txq);
        if (ret != 0)
            rte_exit(EXIT_FAILURE, "Port %u init failed\n", port_id);
    }

    printf("\nRunning rx-only poll loop on %u ports. Ctrl+C to stop.\n\n", nb_ports);

    /* Main poll loop */
    uint64_t total_rx = 0;
    uint64_t last_print = 0;
    uint64_t hz = rte_get_tsc_hz();
    uint64_t print_interval = hz * 5; /* every 5 seconds */
    uint64_t last_tsc = rte_rdtsc();

    while (keep_running) {
        RTE_ETH_FOREACH_DEV(port_id) {
            for (uint16_t q = 0; q < nb_rxq; q++) {
                struct rte_mbuf *bufs[BURST_SIZE];
                uint16_t nb_rx = rte_eth_rx_burst(port_id, q, bufs, BURST_SIZE);
                if (nb_rx > 0) {
                    total_rx += nb_rx;
                    /* Free received mbufs — we're just counting */
                    for (uint16_t i = 0; i < nb_rx; i++)
                        rte_pktmbuf_free(bufs[i]);
                }
            }
        }

        uint64_t now = rte_rdtsc();
        if (now - last_tsc >= print_interval) {
            uint64_t delta_rx = total_rx - last_print;
            double elapsed = (double)(now - last_tsc) / hz;
            printf("  total_rx: %lu  (+%lu in %.1fs = %.0f pps)\n",
                   (unsigned long)total_rx,
                   (unsigned long)delta_rx,
                   elapsed,
                   delta_rx / elapsed);
            last_print = total_rx;
            last_tsc = now;
        }
    }

    printf("\nShutting down...\n");
    RTE_ETH_FOREACH_DEV(port_id) {
        rte_eth_dev_stop(port_id);
        rte_eth_dev_close(port_id);
    }
    rte_eal_cleanup();
    printf("Done. Total packets received: %lu\n", (unsigned long)total_rx);
    return 0;
}
