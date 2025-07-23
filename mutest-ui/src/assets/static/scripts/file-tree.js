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
        this.showBtn = document.getElementById('left-pane-show-btn');
        this.hideBtn = document.getElementById('left-pane-hide-btn');

        this.fileTreeTabBtn = document.getElementById('file-tree-tab-btn');
        this.fileTreeTab = document.getElementById('file-tree-tab');
        this.tracesTabBtn = document.getElementById('traces-tab-btn');
        this.tracesTab = document.getElementById('traces-tab');
    }

    show() {
        this.wrapper.classList.remove('hidden');
        this.showBtn.classList.add('hidden');
    }

    hide() {
        this.wrapper.classList.add('hidden');
        this.showBtn.classList.remove('hidden');
    }

    hideTab(btn, tab) {
        btn.classList.remove('selected');
        tab.classList.add('hidden');
    }

    hideAllTabs() {
        this.hideTab(this.fileTreeTabBtn, this.fileTreeTab);
        this.hideTab(this.tracesTabBtn, this.tracesTab);
    }

    showTab(btn, tab) {
        btn.classList.add('selected');
        tab.classList.remove('hidden');
    }

    showFileTreeTab() {
        this.hideAllTabs();
        this.showTab(this.fileTreeTabBtn, this.fileTreeTab);
    }

    showTracesTab() {
        this.hideAllTabs();
        this.showTab(this.tracesTabBtn, this.tracesTab);
    }

    /**
     * returns the file path associated with a parent element of the mutation in the file tree.
     * @param {HTMLElement} e
     * @returns {string}
     */
    mutationFilePath(e) {
        return [...e.parentElement.parentElement.getElementsByClassName('file')][0].href;
    }

    init() {
        [...this.el.getElementsByClassName('toggle')].map(e => {
            e.addEventListener('click', () => fileNodeToggle(e));
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

        this.fileTreeTabBtn.addEventListener('click', () => {
            this.showFileTreeTab();
        });

        this.tracesTabBtn.addEventListener('click', () => {
            this.showTracesTab();
        });
    }
}

document.addEventListener('DOMContentLoaded', function() {
    let ft = new FileTree(
        document.getElementById('file-tree-wrapper'),
        document.getElementById('file-tree'));
    ft.init();
});

export { FileTree };
