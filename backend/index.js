import { App } from '@tinyhttp/app'
import { logger } from '@tinyhttp/logger'
import { json } from 'milliparsec'
import fs from 'fs'
import YAML from 'yaml'

import { oraPromise } from 'ora'
import { BingChat } from 'bing-chat'

// random choice from an array
function random_choice(arr) {
  return arr[Math.floor(Math.random() * arr.length)]
}
const timeout = (prom, ms) =>
	Promise.race([prom, new Promise((_r, rej) => setTimeout(rej, ms))]);

const BING_COOKIES = YAML.parse(fs.readFileSync('./cookies.yaml', 'utf8')).cookies

// dictionary of convo id to partial response
var qIdToResp = {}

const app = new App()
app
  .use(logger())
  .use(json())
  .post(
    '/newbing/query',
    async (req, res) => {
      // prompt: What is the temperature now in Beijing?
      // cookie: ...
      const bing_cookie = req.body.cookie ? req.body.cookie : random_choice(BING_COOKIES)
      const api = new BingChat({ cookie: bing_cookie })
      const ans = await oraPromise(api.sendMessage(req.body.prompt), {
        text: req.body.prompt
      })
      console.log(ans.text)
      res.status(200).json({
        resp: ans,
        cookie: bing_cookie,
      })
    }
  )
  .post(
    '/newbing/convo',
    async (req, res) => {
      // prompt: What is the temperature now in Beijing?
      // last_resp: {...}
      // cookie: ...
      const bing_cookie = req.body.cookie ? req.body.cookie : random_choice(BING_COOKIES)
      const api = new BingChat({ cookie: bing_cookie })
      console.log(req.body.last_resp)
      const ans = await oraPromise(
        api.sendMessage(req.body.prompt, req.body.last_resp), {
          text: req.body.prompt
        }
      )
      console.log(ans.text)
      res.status(200).json({
        resp: ans,
        cookie: bing_cookie,
      })
    }
  )
  .post(
    '/newbing/onprogress',
    async (req, res) => {
      // prompt: What is the temperature now in Beijing?
      // last_resp: {...}
      // cookie: ...
      const bing_cookie = req.body.cookie ? req.body.cookie : random_choice(BING_COOKIES)
      const api = new BingChat({ cookie: bing_cookie })
      if (req.body.last_resp === undefined || !('id' in req.body.last_resp)) { // new query
        // NOTE: delete id when sending a new question using the same conversation
        console.log('prompt: ' + req.body.prompt)
        var firstSent = false;
        var sendOpts = req.body.last_resp;
        var partialRespId = undefined;
        sendOpts.onProgress = (partialResp) => {
          partialResp.done = false
          partialRespId = partialResp.id;
          qIdToResp[partialRespId] = partialResp
          if (!firstSent) {
            firstSent = true
            res.status(200).json({
              resp: partialResp,
              cookie: bing_cookie,
            })
          }
        }
        timeout(api.sendMessage(req.body.prompt, sendOpts), 60 * 1000)
          .then((resp) => {
            resp.done = true
            qIdToResp[resp.id] = resp
            console.log(resp.text)
            console.log(`done with ${resp.id} successfully`)
          })
          .catch(error => {
            if (partialRespId !== undefined) {
              qIdToResp[partialRespId].done = true
              console.log(`error with ${partialRespId}:`)
            }
            console.log(error)
            if (!firstSent) {
              res.status(200).json({
                err: error,
              })
            }
          })
      }
      else {
        const qid = req.body.last_resp.id;
        if (!(qid in qIdToResp)) {
          res.status(200).json({
            err: `qid ${qid} not found`
          })
        }
        else {
          const resp = qIdToResp[qid]
          res.status(200).json({
            resp: resp,
            cookie: bing_cookie,
          })
          if (resp.done) {
            delete qIdToResp[qid]
          }
        }
      }
    }
  )
  .listen(3000, () => console.log(`Listening on http://localhost:3000`))
