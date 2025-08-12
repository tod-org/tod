// commitlint.config.mjs
export default {
    extends: ['@commitlint/config-conventional'],
    rules: {
        'header-max-length': [2, 'always', 250], //Override the default header line length to 100 characters
        'body-max-line-length': [2, 'always', 250], //Override the default body line length to 250 character
        'body-max-length': [0, 'always', 5000], //Disable the default body length
    },
};
