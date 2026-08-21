// zero-trust gateway dashboard
// sse -> metrics -> chart.js

document.addEventListener('DOMContentLoaded', () => {
    // clock
    function updateClock() {
        const now = new Date();
        const utcString = now.toUTCString().split(' ')[4] + ' UTC';
        document.getElementById('clock').innerText = utcString;
    }
    setInterval(updateClock, 1000);
    updateClock();

    // chart setup
    const ctx = document.getElementById('trafficChart').getContext('2d');
    const maxDataPoints = 30;

    const chartData = {
        labels: Array(maxDataPoints).fill(''),
        datasets: [
            {
                label: 'total requests',
                borderColor: '#5ccfe6',
                backgroundColor: 'rgba(92, 207, 230, 0.08)',
                borderWidth: 1.5,
                tension: 0.3,
                fill: true,
                pointRadius: 0,
                pointHoverRadius: 3,
                data: Array(maxDataPoints).fill(0)
            },
            {
                label: 'active in-flight',
                borderColor: '#7ec699',
                backgroundColor: 'rgba(126, 198, 153, 0.08)',
                borderWidth: 1.5,
                tension: 0.3,
                fill: true,
                pointRadius: 0,
                pointHoverRadius: 3,
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
                duration: 300,
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
                        color: '#777',
                        font: { family: 'JetBrains Mono', size: 11 },
                        usePointStyle: true,
                        boxWidth: 6,
                        padding: 16
                    }
                },
                tooltip: {
                    backgroundColor: '#1a1a1a',
                    titleColor: '#d4d4d4',
                    bodyColor: '#999',
                    borderColor: '#2a2a2a',
                    borderWidth: 1,
                    padding: 8,
                    bodyFont: { family: 'JetBrains Mono', size: 11 },
                    titleFont: { family: 'JetBrains Mono', size: 11 }
                }
            },
            scales: {
                x: {
                    grid: {
                        color: 'rgba(255, 255, 255, 0.03)',
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
                        color: '#555',
                        font: { family: 'JetBrains Mono', size: 10 },
                        precision: 0
                    }
                }
            }
        }
    });

    // sse connection
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
            statusLabel.innerText = 'connected';
        };

        eventSource.onmessage = (event) => {
            try {
                const metrics = JSON.parse(event.data);
                updateUI(metrics);
            } catch (err) {
                console.error('sse parse error:', err);
            }
        };

        eventSource.onerror = (err) => {
            statusIndicator.className = 'status-indicator offline';
            statusLabel.innerText = 'reconnecting...';
            console.warn('sse disconnected, retrying...', err);
        };
    }

    // update ui
    function updateUI(metrics) {
        document.getElementById('total-requests').innerText = Number(metrics.total_requests).toLocaleString();
        document.getElementById('active-requests').innerText = Number(metrics.active_requests).toLocaleString();
        document.getElementById('total-errors').innerText = Number(metrics.total_errors).toLocaleString();
        document.getElementById('ai-cache-hits').innerText = Number(metrics.ai_cache_hits).toLocaleString();
        document.getElementById('ai-cache-misses').innerText = Number(metrics.ai_cache_misses).toLocaleString();

        const totalAi = metrics.ai_cache_hits + metrics.ai_cache_misses;
        let hitRatio = 0;
        if (totalAi > 0) {
            hitRatio = (metrics.ai_cache_hits / totalAi) * 100;
        }

        const hitRatioStr = hitRatio.toFixed(1) + '%';
        document.getElementById('cache-hit-ratio').innerText = hitRatioStr;
        document.getElementById('efficiency-percent').innerText = hitRatioStr;
        document.getElementById('efficiency-bar').style.width = hitRatio.toFixed(1) + '%';

        // update chart
        const now = new Date();
        const timeLabel = now.toTimeString().split(' ')[0];

        chartData.labels.push(timeLabel);
        chartData.datasets[0].data.push(metrics.total_requests);
        chartData.datasets[1].data.push(metrics.active_requests);

        if (chartData.labels.length > maxDataPoints) {
            chartData.labels.shift();
            chartData.datasets[0].data.shift();
            chartData.datasets[1].data.shift();
        }

        trafficChart.update('none');
    }

    connectSSE();
});
