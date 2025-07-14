/**
 * Query.js
 *
 * MIT License
 * Author: SecretSheppy
 * GitHub: https://github.com/SecretSheppy/query-js/tree/main
 */

class Query {

    /**
     * # Query
     *
     * Query takes a query string and stores it in an object. It allows for individual parameters
     * to be changed and then for the query string to be reassembled. It also provides static methods
     * for retrieving the base URL and the query string.
     *
     * @param {string} query the query string to be parsed. Does not have to be from a URL, but should
     * be in the form `x=1&y=2&z=3`...
     */
    constructor(query) {
        this.query = Query.parser(query);
    }

    /**
     * Get the base url
     *
     * @returns {string} the base url of the current page.
     */
    static base() {
        return window.location.origin + window.location.pathname;
    }

    /**
     * The query string
     *
     * @returns {string} the query string of the current page.
     */
    static queryString() {
        return window.location.search.substring(1);
    }

    /**
     * The parser to parse queries. Returns queries in JSON format.
     *
     * @param {string} query the query string to be parsed. Does not have to be from a URL, but should
     * be in the form `x=1&y=2&z=3`...
     * @returns {JSON} the parsed query string.
     */
    static parser(query) {
        let queryObject = {}

        query.split('&').forEach(query => {
            if (query !== '') {
                let splitQuery = query.split('=');
                queryObject[splitQuery[0]] = splitQuery[1];
            }
        });

        return queryObject;
    }

    /**
     * Set an attribute of the query string.
     *
     * @param {string} attribute the attribute to be set. This method creates undefined attributes,
     * so can be used to add new attributes to the string.
     * @param {string} value the value to be set.
     */
    setAttribute(attribute, value) {
        this.query[attribute] = value;
    }

    /**
     * Gets an attribute from the query string.
     *
     * @param {string} attribute the attribute to get.
     * @returns {string|undefined} the value of the attribute. returns undefined if the attribute
     * does not exist within the query string.
     */
    getAttribute(attribute) {
        return this.query[attribute];
    }

    /**
     * Gets an attribute as a boolean from the query string.
     *
     * @param {string} attribute the attribute to fetch from the query string.
     * @returns {boolean} returns true only if the value is true. For all other values, including
     * undefined, false will be returned.
     */
    getBooleanAttribute(attribute) {
        return this.query[attribute] === 'true';
    }

    /**
     * Converts the query string object into the query string.
     *
     * @returns {string} the compiled query string.
     */
    toString() {
        return Object.keys(this.query).map(key => `${key}=${this.query[key]}`).join('&');
    }

}

export { Query };
