import { Query } from "./query.js";

/**
 * opens a file path with a query string
 * @param {string} filePath
 * @param {Query} query
 */
function openFile(filePath, query = new Query('')) {
    window.open(`${filePath}?${query.toString()}`, '_self');
}

export { openFile };