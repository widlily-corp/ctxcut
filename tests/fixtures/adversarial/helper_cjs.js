function calculateDiscount(price, pct) {
    return price * (1 - pct);
}

module.exports = {
    calculateDiscount,
};
