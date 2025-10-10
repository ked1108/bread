document.addEventListener('DOMContentLoaded', function() {
    const searchInput = document.getElementById('search-input');
    const tagFilter = document.getElementById('tag-filter');
    const postsContainer = document.querySelector('.posts-container');
    const postItems = document.querySelectorAll('.post-item');
    const tags = [...new Set(Array.from(document.querySelectorAll('.clickable-tag'))
        .map(t => t.dataset.tag))];

    // Populate tag filter
    const tagFilterSelect = document.getElementById('tag-filter');
    tags.forEach(tag => {
        const option = document.createElement('option');
        option.value = tag;
        option.textContent = '#' + tag;
        tagFilterSelect.appendChild(option);
    });

    function filterPosts() {
        const searchTerm = searchInput.value.toLowerCase();
        const selectedTag = tagFilter.value;

        postItems.forEach(item => {
            const title = item.querySelector('h3 a').textContent.toLowerCase();
            const tags = Array.from(item.querySelectorAll('.clickable-tag'))
            .map(t => t.dataset.tag);

            const matchesSearch = title.includes(searchTerm);
            const matchesTag = !selectedTag || tags.includes(selectedTag);

            item.style.display = matchesSearch && matchesTag ? 'block' : 'none';
        });
    }

    searchInput.addEventListener('input', filterPosts);
    tagFilter.addEventListener('change', filterPosts);

    // Click on tag to filter
    document.querySelectorAll('.clickable-tag').forEach(tag => {
        tag.addEventListener('click', function() {
            tagFilter.value = this.dataset.tag;
            filterPosts();
        });
    });
});
