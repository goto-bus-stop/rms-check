const path = require('path')
const { TextDecoder, promisify } = require('util')
const zip = require('./store-zip')
const { commands, window, workspace, FileSystemError, FileType, Uri } = require('vscode')
const { LanguageClient, TransportKind } = require('vscode-languageclient')

let client = null
let decoder = null

const configuration = workspace.getConfiguration('rmsCheck')
let storagePath = null

const major = process.version.match(/^v(\d+)/)[1]
const defaultUseWasm = parseInt(major, 10) >= 10

const useWasm = configuration.server === 'native' ? false
  : configuration.server === 'wasm' ? true
  : defaultUseWasm

function getWasmServerOptions () {
  return {
    run: {
      module: require.resolve('../server'),
      transport: TransportKind.stdio
    }
  }
}

function getNativeServerOptions () {
  let localServer = path.join(__dirname, '../../target/debug/rms-check')
  try {
    fs.accessSync(localServer)
  } catch (err) {
    localServer = null
  }

  return {
    run: {
      command: 'rms-check',
      args: ['server'],
      transport: TransportKind.stdio
    },
    debug: localServer && {
      command: localServer,
      args: ['server'],
      transport: TransportKind.stdio
    }
  }
}

async function editZrMap (uri) {
  const basename = path.basename(uri.fsPath)
  const bytes = await workspace.fs.readFile(uri)
  const files = zip.read(bytes)

  const mainFile = files.find((f) => /\.rms$/.test(f.header.name))
  if (mainFile) {
    const doc = await workspace.openTextDocument(toZrUri(uri, mainFile.header.name))
    await window.showTextDocument(doc)
  }
}

exports.activate = function activate (context) {
  const serverOptions = useWasm ? getWasmServerOptions() : getNativeServerOptions()
  const clientOptions = {
    documentSelector: ['aoe2-rms']
  }

  decoder = new TextDecoder()
  storagePath = context.storagePath

  client = new LanguageClient('rmsCheck', 'rms-check', serverOptions, clientOptions)
  client.start()

  context.subscriptions.push(commands.registerCommand('rms-check.edit-zr-map', async (uri) => {
    try {
      await editZrMap(uri)
    } catch (err) {
      window.showErrorMessage(err.stack)
    }
  }))
  context.subscriptions.push(workspace.registerFileSystemProvider('aoe2-rms-zr', new ZipRmsFileSystemProvider(), {
    isCaseSensitive: true,
    isReadonly: false
  }))
}

function toZrUri (uri, filename = '') {
  return uri.with({ scheme: 'aoe2-rms-zr', path: `${uri.path}/${filename}` })
}
function toFileUri (uri) {
  let path = uri.path
  const lastSlash = path.lastIndexOf('/')
  const secondToLastSlash = path.lastIndexOf('/', lastSlash - 1)
  if (lastSlash !== -1 && secondToLastSlash !== -1) {
    const filename = path.slice(lastSlash + 1)
    path = path.slice(0, lastSlash)
    return [uri.with({ scheme: 'file', path }), filename]
  }
}

class ZipRmsFileSystemProvider {
  onDidChangeFile (listener) {
    // Ignore for now, should watch the zip file and check entry mtimes in the future
  }

  createDirectory () {
    throw FileSystemError.NoPermissions('ZR@-maps cannot contain directories')
  }

  delete (uri, options) {
    throw FileSystemError.Unavailable('not yet implemented')
  }

  async readDirectory (uri) {
    const [zipFile, filename] = toFileUri(uri)

    const bytes = await workspace.fs.readFile(zipFile)
    const files = zip.read(bytes)

    return files.map((f) => {
      return [f.header.name, FileType.File]
    })
  }

  async readFile (uri) {
    const [zipFile, filename] = toFileUri(uri)

    const bytes = await workspace.fs.readFile(zipFile)
    const files = zip.read(bytes)

    const file = files.find((f) => f.header.name === filename)
    if (!file) {
      throw FileSystemError.FileNotFound(uri)
    }

    return file.data
  }

  rename (oldUri, newUri, options) {
    throw FileSystemError.Unavailable('not yet implemented')
  }

  async stat (uri) {
    const [zipFile, filename] = toFileUri(uri)

    const bytes = await workspace.fs.readFile(zipFile)
    const files = zip.read(bytes)

    const file = files.find((f) => f.header.name === filename)
    if (!file) {
      throw FileSystemError.FileNotFound(uri)
    }

    // TODO implement this part
    const mtime = fromDosDateTime(file.header.mdate, file.header.mtime)

    return {
      ctime: +mtime,
      mtime: +mtime,
      size: file.uncompressedSize,
      type: FileType.File
    }
  }

  watch (uri, options) {
    // throw FileSystemError.Unavailable('not yet implemented')
  }

  async writeFile (uri, content, options) {
    const [zipFile, filename] = toFileUri(uri)

    const bytes = await workspace.fs.readFile(zipFile)
    const files = zip.read(bytes)

    const file = files.find((f) => f.header.name === filename)
    if (!file) {
      throw FileSystemError.FileNotFound(uri)
    }

    ;[file.header.mdate, file.header.mtime] = toDosDateTime(new Date())
    file.data = content

    const newBytes = zip.write(files)
    await workspace.fs.writeFile(zipFile, newBytes)
  }
}

function toDosDateTime (date) {
  return [
    (date.getHours() << 11) + (date.getMinutes() << 5) + (date.getSeconds() / 2),
    ((date.getFullYear() - 1980) << 9) + ((date.getMonth() + 1) << 5) + date.getDate()
  ]
}

function fromDosDateTime (date, time) {
  const year = (date >> 9) + 1980
  const month = ((date >> 5) & 0xF)
  const day = (date & 0x1F)
  const hour = (time >> 11)
  const min = ((time >> 5) & 0x3F)
  const sec = (time & 0x1F) * 2

  return new Date(year, month, day, hour, min, sec)
}

exports.deactivate = function deactivate () {
  if (client) {
    client.stop()
    client = null
  }
}
