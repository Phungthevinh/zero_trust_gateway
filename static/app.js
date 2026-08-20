// =====================================================================
// Zero-Trust API Gateway Dashboard Application Script
// Connects to /admin/events via SSE and updates UI & Chart.js
// =====================================================================

document.addEventListener('DOMContentLoaded', () => {
    // 1. Clock Tracker
    function updateClock() {
        const now = new Date();
        const utcString = now.toUTCString().split(' ')[4] + ' UTC';
        document.getElementById('clock').innerText = utcString;
    }
    setInterval(updateClock, 1000);
    updateClock();

    // 2. Setup Chart.js Real-time Graph
    const ctx = document.getElementById('trafficChart').getContext('2d');
    const maxDataPoints = 30;

    // Gradient fills
    const cyanGradient = ctx.createLinearGradient(0, 0, 0, 300);
    cyanGradient.addColorStop(0, 'rgba(0, 240, 255, 0.35)');
    cyanGradient.addColorStop(1, 'rgba(0, 240, 255, 0.0)');

    const emeraldGradient = ctx.createLinearGradient(0, 0, 0, 300);
    emeraldGradient.addColorStop(0, 'rgba(16, 185, 129, 0.35)');
    emeraldGradient.addColorStop(1, 'rgba(16, 185, 129, 0.0)');

    const chartData = {
        labels: Array(maxDataPoints).fill(''),
        datasets: [
            {
                label: 'Total Requests',
                borderColor: '#00f0ff',
                backgroundColor: cyanGradient,
                borderWidth: 2,
                tension: 0.35,
                fill: true,
                pointRadius: 2,
                pointHoverRadius: 5,
                data: Array(maxDataPoints).fill(0)
            },
            {
                label: 'Active In-Flight Requests',
                borderColor: '#10b981',
                backgroundColor: emeraldGradient,
                borderWidth: 2,
                tension: 0.35,
                fill: true,
                pointRadius: 2,
                pointHoverRadius: 5,
                data: Array(maxDataPoints).fill(0)
            }
        ]
    };

    const trafficChart = new Chart(ctx, {
        type: 'line',
        data: chartData,
        options: {
            responsive: true,
            maintainAspectRatio: false,
            animation: {
                duration: 400,
                easing: 'linear'
            },
            interaction: {
                intersect: false,
                mode: 'index'
            },
            plugins: {
                legend: {
                    position: 'top',
                    labels: {
                        color: '#94a3b8',
                        font: { family: 'Inter', size: 12 },
                        usePointStyle: true,
                        boxWidth: 8
                    }
                },
                tooltip: {
                    backgroundColor: 'rgba(17, 24, 39, 0.95)',
                    titleColor: '#f8fafc',
                    bodyColor: '#cbd5e1',
                    borderColor: 'rgba(255, 255, 255, 0.1)',
                    borderWidth: 1,
                    padding: 10,
                    bodyFont: { family: 'JetBrains Mono' }
                }
            },
            scales: {
                x: {
                    grid: {
                        color: 'rgba(255, 255, 255, 0.04)',
                        drawBorder: false
                    },
                    ticks: { display: false }
                },
                y: {
                    beginAtZero: true,
                    grid: {
                        color: 'rgba(255, 255, 255, 0.04)',
                        drawBorder: false
                    },
                    ticks: {
                        color: '#64748b',
                        font: { family: 'JetBrains Mono', size: 11 },
                        precision: 0
                    }
                }
            }
        }
    });

    // 3. Connect to SSE Endpoint (/admin/events)
    let eventSource = null;
    const statusIndicator = document.getElementById('connection-status');
    const statusLabel = document.getElementById('status-label');

    function connectSSE() {
        if (eventSource) {
            eventSource.close();
        }

        eventSource = new EventSource('/admin/events');

        eventSource.onopen = () => {
            statusIndicator.className = 'status-indicator online';
            statusLabel.innerText = 'SSE CONNECTED';
        };

        eventSource.onmessage = (event) => {
            try {
                const metrics = JSON.parse(event.data);
                updateUI(metrics);
            } catch (err) {
                console.error('Error parsing SSE metrics:', err);
            }
        };

        eventSource.onerror = (err) => {
            statusIndicator.className = 'status-indicator offline';
            statusLabel.innerText = 'RECONNECTING...';
            console.warn('SSE Disconnected, retrying...', err);
        };
    }

    // 4. Update UI Elements & Chart
    function updateUI(metrics) {
        // Cập nhật thẻ chỉ số chính
        document.getElementById('total-requests').innerText = Number(metrics.total_requests).toLocaleString();
        document.getElementById('active-requests').innerText = Number(metrics.active_requests).toLocaleString();
        document.getElementById('total-errors').innerText = Number(metrics.total_errors).toLocaleString();
        document.getElementById('ai-cache-hits').innerText = Number(metrics.ai_cache_hits).toLocaleString();
        document.getElementById('ai-cache-misses').innerText = Number(metrics.ai_cache_misses).toLocaleString();

        // Tính toán tỉ lệ AI Cache Savings
        const totalAi = metrics.ai_cache_hits + metrics.ai_cache_misses;
        let hitRatio = 0;
        if (totalAi > 0) {
            hitRatio = (metrics.ai_cache_hits / totalAi) * 100;
        }

        const hitRatioStr = hitRatio.toFixed(1) + '%';
        document.getElementById('cache-hit-ratio').innerText = hitRatioStr;
        document.getElementById('efficiency-percent').innerText = hitRatioStr;
        document.getElementById('efficiency-bar').style.width = hitRatio.toFixed(1) + '%';

        // Cập nhật biểu đồ Real-time
        const now = new Date();
        const timeLabel = now.toTimeString().split(' ')[0];

        // Shift mảng dữ liệu
        chartData.labels.push(timeLabel);
        chartData.datasets[0].data.push(metrics.total_requests);
        chartData.datasets[1].data.push(metrics.active_requests);

        if (chartData.labels.length > maxDataPoints) {
            chartData.labels.shift();
            chartData.datasets[0].data.shift();
            chartData.datasets[1].data.shift();
        }

        trafficChart.update('none'); // Cập nhật mượt mà không re-render toàn bộ
    }

    // Khởi chạy kết nối
    connectSSE();
});
