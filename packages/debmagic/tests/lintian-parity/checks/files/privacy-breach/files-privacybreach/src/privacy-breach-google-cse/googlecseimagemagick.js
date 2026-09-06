// from imagemagick package
google.load('search', '1', {language : 'en'});
google.setOnLoadCallback(function() {
var customSearchControl = new google.search.CustomSearchControl('002546770416725192290:gpkm0aiuqzq');
customSearchControl.setResultSetSize(google.search.Search.FILTERED_CSE_RESULTSET);
var options = new google.search.DrawOptions();
options.setAutoComplete(true);
customSearchControl.draw('cse', options);
}, true);
