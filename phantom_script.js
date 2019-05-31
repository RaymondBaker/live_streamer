
var page = require('webpage').create();
var args = require('system').args;
var url  = args[1];
var loadInProgress = false;

page.settings.cookiesEnabled = true;
page.settings.userAgent = 'Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/44.0.2403.157 Safari/537.36';
page.settings.javascriptEnabled = true;
//page.navigationLocked = true;

page.onInitialized = function () {
  page.evaluate(function () {
    var create = document.createElement;
    document.createElement = function (tag) {
      var elem = create.call(document, tag);
      if (tag === "video") {
        elem.canPlayType = function () { return "probably" };
      }
      return elem;
    };
    window.navigator = {
      plugins: { "Shockwave Flash": { description: "Shockwave Flash 11.2 e202" } },
      mimeTypes: { "application/x-shockwave-flash": { enabledPlugin: true } }
    };
  });
};

page.onLoadStarted = function() {
    loadInProgress = true;
    console.log('Loading started');
};
page.onLoadFinished = function() {
    loadInProgress = false;
    console.log('Loading finished');
};
page.onResourceRequested = function(request) {
  if (!loadInProgress)
    console.log('Request ' + JSON.stringify(request.url, undefined, 4));
};
page.onResourceReceived = function(response) {
  if (!loadInProgress)
    console.log('Receive ' + JSON.stringify(response.url, undefined, 4));
};
page.onConsoleMessage = function(msg, lineNum, sourceId) {
  console.log('CONSOLE: ' + msg + ' (from line #' + lineNum + ' in "' + sourceId + '")');
};

var steps = [
    function() {
        page.open(url, function(status) {
            console.log("Step 0: Open Page");
            console.log('Status: ' + status);
        });
    },
    function() {
        console.log("Step 1: Click all elements");
        console.log(page.evaluate(function () {
            var out = 0;
            var allElements = document.getElementsByTagName("*");
            var len = allElements.length;
            for (var i = 0; i < len; i++) {
                console.log(allElements[i]);
                if (typeof allElements[i].click === "function")
                {
                    console.log(allElements[i]);
                    allElements[i].click();
                    console.log(window.location.href);
                    out ++;
                }
            }
            return "Done"
        }));
    }
];

interval = setInterval(executeRequestsStepByStep,50);
var testindex=0;
 
function executeRequestsStepByStep(){
    if (loadInProgress == false && typeof steps[testindex] == "function") {
        //console.log("step " + (testindex + 1));
        steps[testindex]();
        testindex++;
    }
    if (typeof steps[testindex] != "function") {
        console.log("test complete!");
        page.render('github.png');
        phantom.exit();
    }
}

