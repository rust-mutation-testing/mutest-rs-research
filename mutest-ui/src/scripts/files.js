/**
 * opens a file path with a query string
 * @param {string} filePath
 * @param {Object} params
 */
function openFile(filePath, params = {}) {
    let formattedParams = [];
    for (let key in Object.keys(params)) {
        formattedParams.push(`${key}=${params[key]}`);
    }
    window.open(`${filePath}?${formattedParams.join('&')}`, '_self');
}

export { openFile };