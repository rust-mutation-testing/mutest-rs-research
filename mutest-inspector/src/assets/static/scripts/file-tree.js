import { openMutation } from "./mutations.js";
import { openFile } from "./files.js";

/**
 * toggles the visibility of a file nodes child elements
 * @param {HTMLElement} fileNodeToggleElement
 */
function fileNodeToggle(fileNodeToggleElement) {
    let toggle = fileNodeToggleElement.parentElement.parentElement;
    if (toggle.classList.contains('expanded')) {
        toggle.classList.remove('expanded');
        return;
    }
    toggle.classList.add('expanded');
}

class FileTree {
    constructor(fileTreeWrapperElement, fileTreeElement) {
        this.wrapper = fileTreeWrapperElement;
        this.el = fileTreeElement;

        // TODO: temp;
        this.showBtn = document.getElementById('file-tree-show-btn');
        this.hideBtn = document.getElementById('file-tree-hide-btn');
    }

    show() {
        this.wrapper.classList.remove('hidden');
        this.showBtn.classList.add('hidden');
    }

    hide() {
        this.wrapper.classList.add('hidden');
        this.showBtn.classList.remove('hidden');
    }

    /**
     * returns the file path associated with a parent element of the mutation in the file tree.
     * @param {HTMLElement} e
     * @returns {string}
     */
    mutationFilePath(e) {
        return [...e.parentElement.parentElement.getElementsByClassName('file')][0]
            .getAttribute('data-file-name');
    }

    init() {
        [...this.el.getElementsByClassName('toggle')].map(e => {
            e.addEventListener('click', () => fileNodeToggle(e));
        });

        [...this.el.getElementsByClassName('file')].map(e => {
            e.addEventListener('click', () => openFile(e.getAttribute('data-file-name')));
        });

        [...this.el.getElementsByClassName('ft-mutation')].map(e => {
            e.addEventListener('click', () => {
                openMutation(e.getAttribute('data-mutation-id'), this.mutationFilePath(e));
            });
        });

        this.showBtn.addEventListener('click', () => {
            this.show();
        });

        this.hideBtn.addEventListener('click', () => {
            this.hide();
        });
    }
}

export { FileTree };
