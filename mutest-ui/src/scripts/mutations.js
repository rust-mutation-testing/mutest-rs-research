import { openFile } from "./files.js";

/**
 * either shows a mutation inline with the code in the current file, or opens the file that contains the mutation,
 * and shows it.
 * @param {number} mutationId
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
        openFile(filePath, { "mutation_id": mutationId });
    }
}

function hideMutationsWithClassName(className) {
    [...document.getElementsByClassName(className)].map(e => e.classList.add('hidden'));
}

export { openMutation, hideMutationsWithClassName };