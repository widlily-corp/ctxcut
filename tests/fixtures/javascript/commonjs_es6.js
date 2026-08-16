function calculateDiscount(price, percentage) {
    if (percentage > 1.0) {
        percentage = percentage / 100.0;
    }
    return price * (1.0 - percentage);
}

function formatCurrency(amount) {
    return "$" + amount.toFixed(2);
}

function processCheckout(itemPrice, discountRate) {
    const finalPrice = calculateDiscount(itemPrice, discountRate);
    return formatCurrency(finalPrice);
}

module.exports = {
    calculateDiscount,
    formatCurrency,
    processCheckout,
};
