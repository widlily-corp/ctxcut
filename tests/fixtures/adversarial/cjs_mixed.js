const { calculateDiscount } = require('./helper_cjs');

function formatCurrency(val) {
    return '$' + val.toFixed(2);
}

function processOrder(price, discountRate) {
    const discounted = calculateDiscount(price, discountRate);
    return formatCurrency(discounted);
}

module.exports = {
    processOrder,
    formatCurrency,
};
