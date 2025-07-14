import { FileTree } from "./file-tree.js";
import { openMutation, hideMutationsWithClassName } from "./mutations.js";

class MutationSwitcher {

    /**
     * Controls the Mutation Switcher portion of the user interface.
     * @param {HTMLElement} mutationSwitcherElement
     * @param {[HTMLElement]} mutationConflictRegionElements
     * @param {FileTree} fileTree
     */
    constructor(mutationSwitcherElement, mutationConflictRegionElements, fileTree) {
        this.el = mutationSwitcherElement;
        // mutation conflict region elements
        this.mcrs = mutationConflictRegionElements;
        this.ft = fileTree;
    }

    show() {
        this.el.classList.remove('hidden');
    }

    hide() {
        this.el.classList.add('hidden');
    }

    /**
     * removes .selected class from all mutation conflict regions
     */
    unselectAllMcrs() {
        this.mcrs.map(e => e.classList.remove('selected'));
    }

    /**
     * populates the changer-regions with the correct mutations based on which mutation region
     * the user clicked.
     * @param {HTMLElement} e
     */
    populateSwitcherContent(e) {
        this.unselectAllMcrs();
        e.classList.add('selected');

        this.show();

        [...document.getElementById('changer-regions').children].map(e => e.classList.add('hidden'));
        document.getElementById(e.classList[0]).classList.remove('hidden');
    }

    showMutationInCode(e) {
        hideMutationsWithClassName(e.getAttribute('data-target-class'));
        openMutation(e.getAttribute('data-mutation-id'));
    }

    init() {
        this.mcrs.map(e => e.addEventListener('click', () => this.populateSwitcherContent(e)));

        [...this.el.getElementsByClassName('mutation-wrapper')].map(e => e.addEventListener('click', () => {
            this.showMutationInCode(e);
        }));

        document.getElementById('mutation-changer-close-btn').addEventListener('click', () => {
            this.hide();
            this.unselectAllMcrs();
        });
    }
}

export { MutationSwitcher };
