import { openFile } from "./files.js";
import { Query } from "./query.js";

const MUTATION_ID_ATTR = 'mutation_id';

/**
 * either shows a mutation inline with the code in the current file, or opens the file that contains the mutation,
 * and shows it.
 * @param {string} mutationId
 * @param {string} filePath
 */
function openMutation(mutationId, filePath = "") {
    try {
        let el = document.getElementById(mutationId);
        if (el.classList.contains('hidden')) {
            let classUuid = el.classList[0];
            hideMutationsWithClassName(classUuid)
            el.classList.remove('hidden');
        }
        [...document.getElementsByTagName('tbody')].map(e => e.classList.remove('selected'));
        el.classList.add('selected');
        el.scrollIntoView();
    } catch (e) {
        console.info(`mutation ${mutationId} not in current file, opening new file`);
        let query = new Query('');
        query.setAttribute(MUTATION_ID_ATTR, mutationId);
        openFile(filePath, query);
    }
}

function hideMutationsWithClassName(className) {
    [...document.getElementsByClassName(className)].map(e => e.classList.add('hidden'));
}

function openQueryMutation() {
    let query = new Query(Query.queryString());
    if (query.getAttribute(MUTATION_ID_ATTR) !== undefined) {
        openMutation(query.getAttribute(MUTATION_ID_ATTR))
    }
}

export { openMutation, hideMutationsWithClassName, openQueryMutation };