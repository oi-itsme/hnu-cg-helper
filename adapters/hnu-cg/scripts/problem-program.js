// 普通编程题（programList.jsp）解析脚本
// 输入：区域提取 + 脱敏后的合成文档（仅含题面/语言选择/上传表单/结果 iframe）
// 输出：ProblemPageOutput { statement_html, statement_text, submission }
//
// 宿主 API：
//   select(css)        → 元素句柄（非负整数），未命中返回 -1
//   selectAll(css)     → 元素句柄数组
//   text(el)           → 元素文本，句柄非法返回 null
//   html(el)           → 元素 innerHTML，句柄非法返回 null
//   attr(el, name)     → 属性值，不存在返回 null
//   cleanHtml(html)    → 白名单清洗（剥除 style/class 等表现层）
//   log(msg)           → 日志

function parse() {
  var contentEl = select('.cgProblemContentClass');
  if (contentEl < 0) throw new Error('未找到题面区域 .cgProblemContentClass');

  // 题面：&nbsp; 归一化为普通空格后做白名单清洗
  var rawHtml = html(contentEl);
  var statementHtml = cleanHtml(rawHtml.replace(/&nbsp;/g, ' '));
  var statementText = text(contentEl).replace(/\s+/g, ' ').trim();

  // 可选语言
  var languages = [];
  var options = selectAll('select#languages option');
  for (var i = 0; i < options.length; i++) {
    languages.push({
      value: attr(options[i], 'value') || '',
      label: text(options[i]).trim()
    });
  }
  if (languages.length === 0) throw new Error('未找到语言选项 select#languages');

  // Java 主类名输入容器存在即表示支持
  var needsMainClass = select('#mainclassObj') >= 0;

  // problemID/assignID 在结果 iframe 的 src 里
  var frame = select('iframe#showmessageFrame');
  if (frame < 0) throw new Error('未找到结果 iframe#showmessageFrame');
  var src = attr(frame, 'src') || '';

  return {
    statement_html: statementHtml,
    statement_text: statementText,
    submission: {
      kind: 'file_upload',
      languages: languages,
      needs_main_class: needsMainClass,
      problem_id: numParam(src, 'problemID'),
      assign_id: numParam(src, 'assignID')
    }
  };
}

function numParam(url, name) {
  var m = new RegExp('[?&]' + name + '=(\\d+)').exec(url);
  if (!m) throw new Error('无法从 `' + url + '` 提取参数 ' + name);
  return parseInt(m[1], 10);
}
